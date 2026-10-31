// SPDX-License-Identifier: MPL-2.0
//! Host KATs for K1a-0: the re-parenting rule (CP-88) and the `CAP_GRAPH`
//! ticket lock (CP-87's writer-starvation bound).

extern crate alloc;

#[path = "../../../kernel/src/sched/cap_graph_logic.rs"]
pub mod cap_graph_logic;

#[path = "../../../kernel/src/sync/ticket_mutex.rs"]
pub mod ticket_mutex;

/// The derivation-edge gate (K1a-0): every kernel site that adds, removes
/// or rewrites a derivation edge (`child -> parent`) must be one of the
/// `CAP_GRAPH` paths in `sched/process.rs`. A new install path that wrote an
/// edge on its own would reopen CP-87 (its parent unchecked under the lock)
/// or CP-88 (an edge dropped without re-parenting); this test names it.
/// Source-level, like the A4 gate; `test_harness.rs` (test fixtures) is
/// exempt.
#[cfg(test)]
mod edge_gate {
    use std::path::{Path, PathBuf};

    /// `(function, mutation)` pairs allowed to touch `derivations`.
    ///
    /// K1b: `derivations` is now `edges::Derivations`, whose every method
    /// takes the caller's `CapGraph`, so the type system is the primary guard.
    /// This scan stays as a second check on where edges are written.
    const ALLOWED: &[(&str, &str)] = &[
        // Installs: each holds CAP_GRAPH (a guard or a `&mut CapGraph`).
        ("install_cap", ".insert_without_cap_graph("), // the q8a negative gate only
        ("install_minted_locked", ".insert("),
        ("install_cap_excluding", ".insert("),
        ("derive_handle", ".insert("),
        ("derive_net_dial_cap", ".insert("),
        // OT-61 C2.2: `spawn_user`'s body is this macro, so that with
        // `mem-quota` off no closure symbol moves (the byte-exact kill-switch).
        ("spawn_user_body", ".insert("),
        // Removals: revoke (whole subtrees) and the two retire paths.
        ("revoke_handle", ".remove_all("),
        ("retire_one", ".remove("),
        ("teardown", ".take("),
        ("retire", ".reparent("),
    ];
    /// The `Derivations` methods that write, plus the map operations a
    /// regression back to a raw map would use.
    const MUTATIONS: &[&str] = &[
        ".insert_without_cap_graph(",
        ".insert(",
        ".remove_all(",
        ".remove(",
        ".take(",
        ".reparent(",
        "take(&mut",
        ".values_mut(",
        ".clear(",
        ".retain(",
    ];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("kernel/src readable") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// The name of the innermost `fn`, or `macro_rules!` body, declared
    /// before `at`.
    fn enclosing_fn(src: &str, at: usize) -> &str {
        let head = &src[..at];
        let f = head.rfind("fn ").map(|i| i + 3);
        let m = head.rfind("macro_rules! ").map(|i| i + 13);
        let i = f.max(m).expect("inside a fn or macro");
        let rest = &head[i..];
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn every_derivation_edge_write_is_a_cap_graph_path() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kernel/src");
        let mut files = Vec::new();
        rust_files(&root, &mut files);
        let mut seen = std::collections::BTreeSet::new();
        for file in files {
            if file.ends_with("test_harness.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&file).unwrap();
            for (at, _) in src.match_indices("derivations") {
                // The expression from this token to the end of its statement.
                let tail = &src[at..];
                let stmt = &tail[..tail.find([';', '{']).unwrap_or(tail.len())];
                // A `mem::take(&mut *x.derivations...)` names its mutation
                // just before the token.
                let before = &src[at.saturating_sub(32)..at];
                let before = &before[before.rfind([';', '{', '}']).map_or(0, |i| i + 1)..];
                let stmt: String = before
                    .split_whitespace()
                    .chain(stmt.split_whitespace())
                    .collect();
                let Some(m) = MUTATIONS
                    .iter()
                    .find(|m| stmt.contains(&m.replace(' ', "")))
                else {
                    continue;
                };
                let f = enclosing_fn(&src, at);
                assert!(
                    ALLOWED.iter().any(|(af, am)| *af == f && am == m),
                    "{}: `{f}` writes a derivation edge ({m}) outside the CAP_GRAPH paths",
                    file.display()
                );
                seen.insert((f.to_string(), m.to_string()));
            }
        }
        // The gate is live: it found the real sites, not nothing.
        for (f, m) in [
            ("derive_handle", ".insert("),
            ("teardown", ".take("),
            ("revoke_handle", ".remove_all("),
        ] {
            assert!(
                seen.contains(&(f.to_string(), m.to_string())),
                "gate missed {f} {m}"
            );
        }
    }

    /// Each allowed function holds the lock: it takes `lock_cap_graph()` or
    /// is a `CapGraph` method / takes a `CapGraph`.
    #[test]
    fn every_allowed_site_holds_cap_graph() {
        let src = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kernel/src/sched/process.rs"),
        )
        .unwrap();
        for (f, _) in ALLOWED {
            let start = src
                .find(&format!("fn {f}("))
                .or_else(|| src.find(&format!("macro_rules! {f} ")))
                .unwrap_or_else(|| panic!("fn {f} gone"));
            let body = &src[start..];
            let body = &body[..body[1..]
                .find("\nfn ")
                .or_else(|| body[1..].find("\npub fn "))
                .unwrap_or(body.len() - 1)
                + 1];
            assert!(
                body.contains("lock_cap_graph()")
                    || body.contains("CapGraph")
                    || body.contains("&mut self"),
                "fn {f} touches derivations without CAP_GRAPH"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cap_graph_logic::{reparent, resolve};
    use super::ticket_mutex::TicketMutex;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// A model capability graph: `cap -> (parent, rights)`. Rights are a bit
    /// set; a derive takes a subset of its parent's.
    #[derive(Clone, Default)]
    struct Graph {
        caps: BTreeMap<u32, (Option<u32>, u8)>,
    }

    impl Graph {
        fn root(&mut self, c: u32, rights: u8) {
            self.caps.insert(c, (None, rights));
        }

        fn derive(&mut self, c: u32, parent: u32, rights: u8) {
            let parent_rights = self.caps[&parent].1;
            assert_eq!(rights & !parent_rights, 0, "a derive never widens");
            self.caps.insert(c, (Some(parent), rights));
        }

        /// Remove `going` at once (one close, or one process's teardown),
        /// re-parenting the rest with the kernel's rule.
        fn retire(&mut self, going: &[u32]) {
            let dying: BTreeMap<u32, Option<u32>> = going
                .iter()
                .map(|c| (*c, self.caps.remove(c).expect("live").0))
                .collect();
            for (parent, _) in self.caps.values_mut() {
                reparent(parent, &dying);
            }
        }

        /// The live ancestors of `c`, following parents.
        fn ancestors(&self, c: u32) -> BTreeSet<u32> {
            let mut out = BTreeSet::new();
            let mut at = self.caps[&c].0;
            while let Some(p) = at {
                assert!(out.insert(p), "cycle");
                at = self.caps.get(&p).expect("a parent is live").0;
            }
            out
        }

        /// What a revoke of `r` removes: `r` and every cap reaching it.
        fn revoke_set(&self, r: u32) -> BTreeSet<u32> {
            self.caps
                .keys()
                .copied()
                .filter(|c| *c == r || self.ancestors(*c).contains(&r))
                .collect()
        }

        fn check(&self, before: &Graph) {
            for (c, (parent, rights)) in &self.caps {
                // Every edge names a live cap (no orphan the walk can't reach).
                if let Some(p) = parent {
                    let (_, parent_rights) = self.caps.get(p).expect("parent live");
                    // Rights stay monotone across a re-parent.
                    assert_eq!(rights & !parent_rights, 0, "cap {c} wider than {p}");
                }
                // Ancestry is preserved: the live ancestors are exactly the
                // old ancestors that are still live.
                let old: BTreeSet<u32> = before
                    .ancestors(*c)
                    .into_iter()
                    .filter(|a| self.caps.contains_key(a))
                    .collect();
                assert_eq!(
                    self.ancestors(*c),
                    old,
                    "cap {c} lost or gained an ancestor"
                );
            }
        }
    }

    /// root(1) -> 2 -> 3 -> 4 -> 5, rights narrowing down the chain.
    fn chain() -> Graph {
        let mut g = Graph::default();
        g.root(1, 0b1111);
        g.derive(2, 1, 0b0111);
        g.derive(3, 2, 0b0111);
        g.derive(4, 3, 0b0011);
        g.derive(5, 4, 0b0001);
        g
    }

    fn permutations(items: &[u32]) -> Vec<Vec<u32>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let x = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, x);
                out.push(p);
            }
        }
        out
    }

    /// The PM's case: a chain of five where the middle caps close one at a
    /// time, in every order and every subset. After each close the graph
    /// keeps every surviving ancestor, rights stay monotone, and revoking
    /// the root still reaches every survivor.
    #[test]
    fn middle_closes_in_any_order_keep_the_tree() {
        let middle = [2u32, 3, 4];
        for mask in 1u32..(1 << middle.len()) {
            let subset: Vec<u32> = middle
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, c)| *c)
                .collect();
            for order in permutations(&subset) {
                let mut g = chain();
                for c in &order {
                    let before = g.clone();
                    g.retire(&[*c]);
                    g.check(&before);
                }
                let all: BTreeSet<u32> = g.caps.keys().copied().collect();
                assert_eq!(g.revoke_set(1), all, "order {order:?}");
            }
        }
    }

    /// The middle two go at once (one process's teardown held both).
    #[test]
    fn a_teardown_of_adjacent_caps_skips_both() {
        let mut g = chain();
        let before = g.clone();
        g.retire(&[3, 2]);
        g.check(&before);
        assert_eq!(g.caps[&4].0, Some(1));
        assert_eq!(g.revoke_set(1).len(), 3);
    }

    /// A tree: two branches under one root, a close in each, plus a
    /// teardown that removes a root of its own (its children become roots).
    #[test]
    fn branches_and_a_dying_root() {
        let mut g = Graph::default();
        g.root(1, 0b111);
        g.derive(2, 1, 0b011);
        g.derive(3, 1, 0b101);
        g.derive(4, 2, 0b001);
        g.derive(5, 3, 0b100);
        g.derive(6, 3, 0b001);
        for going in [&[2u32][..], &[3], &[1]] {
            let before = g.clone();
            g.retire(going);
            g.check(&before);
        }
        assert_eq!(g.caps[&4].0, None);
        assert_eq!(g.caps[&5].0, None);
        assert_eq!(g.caps[&6].0, None);
    }

    #[test]
    fn resolve_edges() {
        let dying: BTreeMap<u32, Option<u32>> = [(2, Some(1)), (3, Some(2)), (7, None)].into();
        assert_eq!(resolve(&dying, 3), Some(1));
        assert_eq!(resolve(&dying, 9), Some(9));
        assert_eq!(resolve(&dying, 7), None);
        // A cycle ends the walk instead of looping.
        let cyclic: BTreeMap<u32, Option<u32>> = [(1, Some(2)), (2, Some(1))].into();
        assert_eq!(resolve(&cyclic, 1), None);
        let mut edge = Some(3);
        assert_eq!(reparent(&mut edge, &dying), Some(Some(1)));
        let mut untouched = Some(9);
        assert_eq!(reparent(&mut untouched, &dying), None);
        assert_eq!(untouched, Some(9));
    }

    /// Mutual exclusion under contention.
    // Janus F3: a guard derefs to `&T`, so sharing it shares `&T`. The guard
    // must be `Sync` only when `T: Sync`. Detected with the inherent-vs-trait
    // associated-const rule: `IsSync::<T>::SYNC` resolves to the inherent
    // `true` only when `T: Sync`, otherwise to the blanket trait's `false`.
    struct IsSync<T: ?Sized>(core::marker::PhantomData<T>);
    trait NotSync {
        const SYNC: bool = false;
    }
    impl<T: ?Sized> NotSync for IsSync<T> {}
    #[allow(dead_code)]
    impl<T: ?Sized + Sync> IsSync<T> {
        const SYNC: bool = true;
    }

    #[test]
    fn ticket_guard_is_sync_only_when_t_is_sync() {
        use super::ticket_mutex::TicketMutexGuard;
        use core::cell::Cell;
        // Checked at compile time: a regression fails this crate's build.
        // `Cell<u32>` is Send but not Sync: the lock is Sync, its guard is not.
        const {
            assert!(IsSync::<super::ticket_mutex::TicketMutex<Cell<u32>>>::SYNC);
            assert!(!IsSync::<TicketMutexGuard<'static, Cell<u32>>>::SYNC);
            // A Sync payload keeps a Sync guard (CAP_GRAPH's `BTreeSet`).
            assert!(IsSync::<TicketMutexGuard<'static, std::collections::BTreeSet<u64>>>::SYNC);
            // The probe itself tells the two apart.
            assert!(!IsSync::<Cell<u32>>::SYNC);
            assert!(IsSync::<u32>::SYNC);
        }
    }

    #[test]
    fn ticket_mutex_excludes() {
        let lock = Arc::new(TicketMutex::new(0u64));
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let lock = lock.clone();
                std::thread::spawn(move || {
                    for _ in 0..20_000 {
                        *lock.lock() += 1;
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(*lock.lock(), 80_000);
        assert!(lock.try_lock().is_some());
    }

    /// The starvation bound: one waiter queued behind a stream of other
    /// lockers is admitted before any locker that took its ticket later.
    #[test]
    fn ticket_mutex_is_fifo() {
        let lock = Arc::new(TicketMutex::new(()));
        let held = lock.lock();
        // A held lock refuses try_lock.
        assert!(lock.try_lock().is_none());
        let order = Arc::new(AtomicU64::new(0));
        let admitted = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        // The "revoke": queued while the lock is held.
        let waiter = {
            let (lock, order, admitted) = (lock.clone(), order.clone(), admitted.clone());
            std::thread::spawn(move || {
                let _g = lock.lock();
                admitted.store(order.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
            })
        };
        while lock.tickets_taken() < 2 {
            std::hint::spin_loop();
        }
        // The "derives": a stream that only starts once the waiter queued.
        let stream: Vec<_> = (0..3)
            .map(|_| {
                let (lock, order, stop) = (lock.clone(), order.clone(), stop.clone());
                std::thread::spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        let _g = lock.lock();
                        order.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        drop(held);
        waiter.join().unwrap();
        stop.store(true, Ordering::SeqCst);
        for t in stream {
            t.join().unwrap();
        }
        assert_eq!(
            admitted.load(Ordering::SeqCst),
            0,
            "the queued waiter goes first"
        );
    }
}
