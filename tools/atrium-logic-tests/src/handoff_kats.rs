// SPDX-License-Identifier: MPL-2.0
//! Chad DS-1a F1 discriminator: the keyboard handoff order against a model
//! of ADR-074 routing (one active route; each owner's queue keeps what it was
//! given while inactive). A key is injected at every step boundary of every
//! handoff in a Home → Arthur → Home → Arthur round trip. The production
//! order must never deliver a key to an owner in a turn other than the one it
//! was typed in, and must deliver every key typed once the new view is on
//! screen. DS-1a's own orders fail the first rule.

use super::atrium_handoff::{HANDOFF, Handoff, Step, hand_over};

#[derive(Clone, Copy, Debug)]
struct Key {
    /// The owner whose route was live when the key was typed, and its turn.
    owner: usize,
    turn: u32,
    /// Typed once the step order had presented the new view.
    after_present: bool,
}

#[derive(Default)]
struct World {
    route: usize,
    turn: [u32; 2],
    queues: [Vec<Key>; 2],
    /// Keys typed once the new view was presented (each must be delivered).
    typed_after: usize,
    /// (owner, turn, key) for every key an owner read.
    delivered: Vec<(usize, u32, Key)>,
}

impl World {
    fn type_key(&mut self, after_present: bool) {
        let owner = self.route;
        let key = Key {
            owner,
            turn: self.turn[owner],
            after_present,
        };
        self.queues[owner].push(key);
        self.typed_after += usize::from(after_present);
    }

    /// The live owner reads everything queued for it.
    fn read(&mut self) {
        let owner = self.route;
        for key in core::mem::take(&mut self.queues[owner]) {
            self.delivered.push((owner, self.turn[owner], key));
        }
    }
}

/// One handoff to `to`, typing a key at the boundaries in `type_at` (0 =
/// before the first step … `order.len()` = after the last).
struct Run<'a> {
    world: &'a mut World,
    to: usize,
    step: usize,
    type_at: &'a [usize],
    presented: bool,
}

impl Run<'_> {
    fn boundary(&mut self) {
        if self.type_at.contains(&self.step) {
            self.world.type_key(self.presented);
        }
        self.step += 1;
    }
}

impl Handoff for Run<'_> {
    type Error = ();
    fn swap(&mut self) -> Result<(), ()> {
        self.boundary();
        self.world.route = self.to;
        self.world.turn[self.to] += 1;
        Ok(())
    }
    fn discard(&mut self) -> Result<(), ()> {
        self.boundary();
        // Only the live route can be received from (ADR-074): an owner that
        // is not yet the route cannot have its queue drained.
        if self.world.route == self.to {
            self.world.queues[self.to].clear();
        }
        Ok(())
    }
    fn present(&mut self) -> Result<(), ()> {
        self.boundary();
        self.presented = true;
        Ok(())
    }
}

/// Run Home(0) → Arthur(1) → Home → Arthur with every combination of one
/// typed key per handoff boundary; returns the first violation.
fn check(order: &[Step]) -> Result<(), String> {
    let points = order.len() + 1;
    for a in 0..points {
        for b in 0..points {
            for c in 0..points {
                let mut world = World::default();
                world.turn[0] = 1;
                for (to, at) in [(1, a), (0, b), (1, c)] {
                    let mut run = Run {
                        world: &mut world,
                        to,
                        step: 0,
                        type_at: &[at],
                        presented: false,
                    };
                    hand_over(&mut run, order).unwrap();
                    if at == order.len() {
                        run.boundary();
                    }
                    world.read();
                }
                for &(owner, turn, key) in &world.delivered {
                    if key.owner != owner || key.turn != turn {
                        return Err(format!(
                            "{order:?} at {:?}: a key typed for owner {} turn {} reached owner {owner} turn {turn}",
                            (a, b, c),
                            key.owner,
                            key.turn
                        ));
                    }
                }
                let typed_after = world.typed_after;
                let got_after = world.delivered.iter().filter(|d| d.2.after_present).count();
                if got_after != typed_after {
                    return Err(format!(
                        "{order:?} at {:?}: {typed_after} keys typed on the new view, {got_after} delivered",
                        (a, b, c)
                    ));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn production_handoff_never_replays_a_key_into_a_later_turn() {
    assert_eq!(HANDOFF, [Step::Swap, Step::Discard, Step::Present]);
    check(&HANDOFF).unwrap();
}

/// Negative controls: DS-1a's hold/resume (view first, then the route) and a
/// route-first order without the discard replay a key into the owner's next
/// turn; a discard before the swap cannot drain (the owner is not yet the
/// route); presenting before the swap drops keys typed on the new view.
#[test]
fn ds1a_orders_fail_the_handoff_model() {
    for bad in [
        &[Step::Present, Step::Swap][..],
        &[Step::Swap, Step::Present][..],
        &[Step::Present, Step::Swap, Step::Discard][..],
        &[Step::Discard, Step::Swap, Step::Present][..],
    ] {
        assert!(check(bad).is_err(), "{bad:?} must fail the model");
    }
}
