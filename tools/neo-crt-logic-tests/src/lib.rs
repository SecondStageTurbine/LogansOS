// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `userland/neo-crt/src/seqcond.rs`, the sequence condvar
//! behind neo-crt's `pthread_cond_t`. The real module is mounted through
//! `#[path]`, so these tests exercise the exact code the target runs.
//!
//! The discriminator (#67 review, Chad, item 2) needs a wake order std's
//! condvar on Linux won't give: our kernel puts a futex waiter in the first
//! free queue slot (`kernel/src/sched/futex.rs`) and wakes the lowest slot
//! first, so a thread that starts waiting late can be woken before one that
//! has waited longer. `SlotCv` models that worst case: `notify_one` wakes the
//! most recently registered waiter. It also gives a late waiter every chance
//! to arrive: `notify_one` first waits (up to `LATE_WAITER`) for one more
//! waiter to register. A `SeqCond` that lets a thread start waiting between
//! bumping the sequence and waking then loses the signal every time; one that
//! wakes under the sequence lock never does.

#![allow(dead_code)]

#[path = "../../../userland/neo-crt/src/seqcond.rs"]
mod seqcond;

#[cfg(test)]
mod tests {
    use super::seqcond::{Cv, SeqCond};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};
    use std::thread;
    use std::time::{Duration, Instant};

    /// How long `SlotCv::notify_one` waits for a late waiter to register.
    const LATE_WAITER: Duration = Duration::from_secs(2);

    #[derive(Default)]
    struct Flag {
        woken: Mutex<bool>,
        cv: Condvar,
    }

    /// A condvar that wakes the newest waiter first (see the module doc).
    #[derive(Default)]
    struct SlotCv {
        queue: Mutex<Vec<Arc<Flag>>>,
        changed: Condvar,
        /// Set when a `notify_one` has started (the sequence is bumped).
        notifying: AtomicBool,
    }

    impl SlotCv {
        fn register(&self) -> Arc<Flag> {
            let f = Arc::new(Flag::default());
            self.queue.lock().unwrap().push(f.clone());
            self.changed.notify_all();
            f
        }

        fn waiting(&self) -> usize {
            self.queue.lock().unwrap().len()
        }

        fn wait_until(&self, what: impl Fn(&Self) -> bool) {
            let end = Instant::now() + Duration::from_secs(10);
            while !what(self) {
                assert!(
                    Instant::now() < end,
                    "model never reached the expected state"
                );
                thread::sleep(Duration::from_millis(1));
            }
        }

        fn wake(f: &Flag) {
            *f.woken.lock().unwrap() = true;
            f.cv.notify_one();
        }
    }

    impl Cv for SlotCv {
        fn wait<'a>(&self, m: &'a Mutex<u64>, g: MutexGuard<'a, u64>) -> MutexGuard<'a, u64> {
            // Registered before the sequence lock is released, as std's
            // condvar reads its futex word before unlocking.
            let f = self.register();
            drop(g);
            let mut w = f.woken.lock().unwrap();
            while !*w {
                w = f.cv.wait(w).unwrap();
            }
            drop(w);
            m.lock().unwrap()
        }

        fn wait_timeout<'a>(
            &self,
            m: &'a Mutex<u64>,
            g: MutexGuard<'a, u64>,
            d: Duration,
        ) -> MutexGuard<'a, u64> {
            let f = self.register();
            drop(g);
            let w = f.woken.lock().unwrap();
            let (w, _) = f.cv.wait_timeout_while(w, d, |w| !*w).unwrap();
            drop(w);
            self.queue.lock().unwrap().retain(|q| !Arc::ptr_eq(q, &f));
            m.lock().unwrap()
        }

        fn notify_one(&self) {
            self.notifying.store(true, Ordering::SeqCst);
            let q = self.queue.lock().unwrap();
            let before = q.len();
            let (mut q, _) = self
                .changed
                .wait_timeout_while(q, LATE_WAITER, |q| q.len() <= before)
                .unwrap();
            if let Some(f) = q.pop() {
                SlotCv::wake(&f);
            }
        }

        fn notify_all(&self) {
            for f in self.queue.lock().unwrap().drain(..) {
                SlotCv::wake(&f);
            }
        }
    }

    /// The order the first version of `pthread_cond_signal` used: bump the
    /// sequence, release the lock, then wake. Kept here only as the
    /// discriminator's control.
    fn signal_after_unlock(sc: &SeqCond<SlotCv>) {
        {
            let mut g = sc.seq.lock().unwrap();
            *g += 1;
        }
        sc.cv.notify_one();
    }

    /// Waiter A waits for `ready.0`; a producer sets it and signals; while the
    /// producer is in `notify_one`, waiter B starts waiting for `ready.1`
    /// (never set until cleanup). Returns whether A got the signal.
    fn a_gets_the_signal(signal: fn(&SeqCond<SlotCv>)) -> bool {
        let sc = Arc::new(SeqCond::new(SlotCv::default()));
        let user = Arc::new(Mutex::new((false, false)));
        let a_done = Arc::new(AtomicBool::new(false));

        let waiter = |idx: usize, done: Option<Arc<AtomicBool>>| {
            let (sc, user) = (sc.clone(), user.clone());
            thread::spawn(move || {
                let mut st = user.lock().unwrap();
                while !(if idx == 0 { st.0 } else { st.1 }) {
                    let held = st;
                    sc.wait(
                        move || {
                            drop(held);
                            Ok::<(), ()>(())
                        },
                        None,
                    )
                    .unwrap();
                    st = user.lock().unwrap();
                }
                if let Some(d) = done {
                    d.store(true, Ordering::SeqCst);
                }
            })
        };

        let a = waiter(0, Some(a_done.clone()));
        sc.cv.wait_until(|cv| cv.waiting() == 1);

        let producer = {
            let (sc, user) = (sc.clone(), user.clone());
            thread::spawn(move || {
                user.lock().unwrap().0 = true;
                signal(&sc);
            })
        };
        sc.cv.wait_until(|cv| cv.notifying.load(Ordering::SeqCst));
        let b = waiter(1, None);

        producer.join().unwrap();
        let end = Instant::now() + Duration::from_secs(1);
        while !a_done.load(Ordering::SeqCst) && Instant::now() < end {
            thread::sleep(Duration::from_millis(5));
        }
        let got = a_done.load(Ordering::SeqCst);

        // Release everyone (A too, if its signal was lost).
        user.lock().unwrap().1 = true;
        while !a.is_finished() || !b.is_finished() {
            sc.broadcast();
            thread::sleep(Duration::from_millis(5));
        }
        a.join().unwrap();
        b.join().unwrap();
        got
    }

    #[test]
    fn signal_reaches_the_waiter_it_was_for() {
        assert!(a_gets_the_signal(SeqCond::signal));
    }

    /// Control: with the wake after the unlock, the late waiter takes the
    /// signal and A sleeps on. If this ever passes, the model no longer
    /// reproduces the race and `signal_reaches_the_waiter_it_was_for` proves
    /// nothing.
    #[test]
    fn control_waking_after_unlock_loses_the_signal() {
        assert!(!a_gets_the_signal(signal_after_unlock));
    }

    #[test]
    fn broadcast_wakes_every_waiter() {
        let sc = Arc::new(SeqCond::new(SlotCv::default()));
        let woke: Vec<_> = (0..3)
            .map(|_| {
                let sc = sc.clone();
                thread::spawn(move || sc.wait(|| Ok::<(), ()>(()), None).unwrap())
            })
            .collect();
        sc.cv.wait_until(|cv| cv.waiting() == 3);
        sc.broadcast();
        for w in woke {
            assert!(!w.join().unwrap(), "no waiter times out");
        }
    }

    #[test]
    fn a_passed_deadline_times_out_after_releasing() {
        let sc = SeqCond::new(Condvar::new());
        let mut released = false;
        let r = sc.wait(
            || {
                released = true;
                Ok::<(), ()>(())
            },
            Some(Instant::now()),
        );
        assert_eq!(r, Ok(true));
        assert!(released);
    }

    #[test]
    fn a_failed_release_is_returned_and_nothing_waits() {
        let sc = SeqCond::new(Condvar::new());
        assert_eq!(sc.wait(|| Err::<(), i32>(1), None), Err(1));
        assert_eq!(*sc.seq.lock().unwrap(), 0);
    }
}
