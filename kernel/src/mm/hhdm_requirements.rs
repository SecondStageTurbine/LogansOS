// SPDX-License-Identifier: MPL-2.0
/// Half-open physical range that must be reachable through HHDM.
#[derive(Copy, Clone)]
pub struct PhysExtent {
    pub start: u64,
    pub end: u64,
}

impl PhysExtent {
    pub const EMPTY: Self = Self { start: 0, end: 0 };

    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Limine module range required by early ELF loading.
#[derive(Copy, Clone)]
pub struct BootModuleExtent {
    pub extent: PhysExtent,
}

impl BootModuleExtent {
    pub const EMPTY: Self = Self {
        extent: PhysExtent::EMPTY,
    };
}

/// HHDM spans that boot code will dereference before userspace starts.
pub struct BootHhdmRequirements<'a> {
    pub executable: PhysExtent,
    pub modules: &'a [BootModuleExtent],
}

/// Snapshot exposed to neo-test after `mm::init`.
#[allow(dead_code)]
pub struct HhdmBootReport {
    pub required_end: u64,
    pub repair_pages: u64,
    pub module_count: u64,
}

/// Compute the HHDM physical extent required by known boot-owned objects.
pub fn required_hhdm_phys_end(
    usable_end: u64,
    executable: PhysExtent,
    modules: &[BootModuleExtent],
) -> u64 {
    let mut end = usable_end.max(executable.end);
    for module in modules {
        end = end.max(module.extent.end);
    }
    end
}

#[cfg(test)]
mod tests {
    use super::{BootModuleExtent, PhysExtent, required_hhdm_phys_end};

    const fn module(start: u64, end: u64) -> BootModuleExtent {
        BootModuleExtent {
            extent: PhysExtent::new(start, end),
        }
    }

    #[test]
    fn required_end_uses_exec_when_no_modules() {
        let exec = PhysExtent::new(0x1000, 0x5000);
        assert_eq!(required_hhdm_phys_end(0x3000, exec, &[]), 0x5000);
    }

    #[test]
    fn required_end_uses_module_above_usable() {
        let exec = PhysExtent::new(0x1000, 0x5000);
        let modules = [module(0x9000, 0xd000)];
        assert_eq!(required_hhdm_phys_end(0x8000, exec, &modules), 0xd000);
    }

    #[test]
    fn required_end_keeps_usable_when_modules_below() {
        let exec = PhysExtent::new(0x1000, 0x5000);
        let modules = [module(0x6000, 0x7000)];
        assert_eq!(required_hhdm_phys_end(0x9000, exec, &modules), 0x9000);
    }

    #[test]
    fn required_end_uses_highest_of_multiple_modules() {
        let exec = PhysExtent::new(0x1000, 0x5000);
        let modules = [module(0x7000, 0x8800), module(0x8000, 0xa000)];
        assert_eq!(required_hhdm_phys_end(0x9000, exec, &modules), 0xa000);
    }

    #[test]
    fn required_end_does_not_overcount_exec_equal_usable() {
        let exec = PhysExtent::new(0x1000, 0x8000);
        let modules = [module(0x2000, 0x3000)];
        assert_eq!(required_hhdm_phys_end(0x8000, exec, &modules), 0x8000);
    }
}
