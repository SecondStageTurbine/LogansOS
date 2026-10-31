// SPDX-License-Identifier: MPL-2.0
//! IRQ-safe mutex for locks shared by process and IF=0 contexts.
//!
//! `IrqSafeMutex` disables local interrupts before acquiring the inner
//! spinlock and restores the prior IF state when the guard is dropped.

#![allow(dead_code)]

use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::AtomicBool;

use crate::arch::x86_64::{disable_interrupts, enable_interrupts, interrupts_enabled};
use crate::sync::lock_owner::LockOwnerSnapshot;
use crate::sync::switch_mutex::{SwitchMutex, SwitchMutexGuard};

pub struct IrqSafeMutex<T> {
    inner: SwitchMutex<T>,
}

pub struct IrqSafeGuard<'a, T> {
    guard: ManuallyDrop<SwitchMutexGuard<'a, T>>,
    prior_if_state: bool,
}

impl<T> IrqSafeMutex<T> {
    #[inline(always)]
    pub const fn new(value: T) -> Self {
        Self {
            inner: SwitchMutex::new(value),
        }
    }

    /// [`SwitchMutex::new_ordered`], IF-safe: a lock-order level for a lock
    /// that must also never be held with interrupts on.
    #[inline(always)]
    pub const fn new_ordered(value: T, order: u8) -> Self {
        Self {
            inner: SwitchMutex::new_ordered(value, order),
        }
    }

    #[track_caller]
    #[inline(always)]
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        let prior_if_state = interrupts_enabled();
        disable_interrupts();
        let guard = self.inner.lock();

        IrqSafeGuard {
            guard: ManuallyDrop::new(guard),
            prior_if_state,
        }
    }

    /// Non-blocking acquire. On failure the caller's prior IF state is
    /// **restored** before returning `None`, so a retry loop does not silently
    /// run with interrupts forced off — which would change the semantics of the
    /// loop body and, worse, mask the very ticks a bounded-spin diagnostic needs
    /// to stay honest about.
    #[track_caller]
    #[inline(always)]
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, T>> {
        let prior_if_state = interrupts_enabled();
        disable_interrupts();
        match self.inner.try_lock() {
            Some(guard) => Some(IrqSafeGuard {
                guard: ManuallyDrop::new(guard),
                prior_if_state,
            }),
            None => {
                if prior_if_state {
                    enable_interrupts();
                }
                None
            }
        }
    }

    /// CP-56 discriminator controls only: the pre-fix acquisition shape (IF
    /// as the caller has it), so a gate can prove its IF=1 witness fires.
    #[cfg(any(
        feature = "neo-test-hid-mixed-if-control",
        feature = "neo-test-hid-device-if-control",
        feature = "neo-test-delivery-drop-control"
    ))]
    #[track_caller]
    pub fn lock_as_caller(&self) -> SwitchMutexGuard<'_, T> {
        self.inner.lock()
    }

    #[allow(dead_code)]
    pub fn owner_snapshot(&self) -> Option<LockOwnerSnapshot> {
        self.inner.owner_snapshot()
    }
}

impl<'a, T> IrqSafeGuard<'a, T> {
    #[allow(dead_code)]
    #[inline(always)]
    pub fn release_token_for_context_switch(&mut self) -> *const AtomicBool {
        debug_assert!(
            !self.prior_if_state,
            "IrqSafeGuard context-switch release requires an IF-off caller"
        );
        self.guard.release_token_for_context_switch()
    }
}

impl<T> Deref for IrqSafeGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T> DerefMut for IrqSafeGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl<T> Drop for IrqSafeGuard<'_, T> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY: `guard` is wrapped in ManuallyDrop and dropped exactly once
        // here, before IF restoration, so the spinlock is released while local
        // interrupts are still masked.
        unsafe {
            ManuallyDrop::drop(&mut self.guard);
        }

        if self.prior_if_state {
            enable_interrupts();
        }
    }
}
