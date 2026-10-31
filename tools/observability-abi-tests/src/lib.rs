// SPDX-License-Identifier: MPL-2.0
//! Host byte oracle for the OT-9 `SYS_OBSERVE` ABI.
//!
//! This crate deliberately does not link `libneo` or kernel code. It is an
//! independent host KAT that pins the source field order and the exact byte
//! offsets a user/kernel ABI consumer must observe.

#[cfg(test)]
mod tests {
    use core::mem::{offset_of, size_of};

    const LIBNEO_SRC: &str = include_str!("../../../userland/libneo/src/lib.rs");
    const KERNEL_OBSERVABILITY_SRC: &str = include_str!("../../../kernel/src/observability.rs");
    const KERNEL_PER_CPU_SRC: &str = include_str!("../../../kernel/src/arch/x86_64/per_cpu.rs");

    const ABI_VERSION: usize = 3;
    const MAX_CPUS: usize = 64;
    const TRACE_EVENTS: usize = 16;
    const SCHEDULER_STATS_SIZE: usize = 72;
    const TRACE_EVENT_SIZE: usize = 40;
    const OBSERVABILITY_SCHED_OFFSET: usize = 112;
    const OBSERVABILITY_TRACE_OFFSET: usize =
        OBSERVABILITY_SCHED_OFFSET + SCHEDULER_STATS_SIZE * MAX_CPUS;
    const OBSERVABILITY_SIZE: usize = OBSERVABILITY_TRACE_OFFSET + TRACE_EVENT_SIZE * TRACE_EVENTS;

    #[repr(C)]
    struct OracleSchedulerStatsV1 {
        cpu_id: u32,
        _pad0: u32,
        context_switches: u64,
        dispatches: u64,
        ready_queue_depth: u64,
        run_ticks: u64,
        idle_ticks: u64,
        timer_preemptions: u64,
        voluntary_yields: u64,
        ready_queue_high_water: u64,
    }

    #[repr(C)]
    struct OracleTraceEventV1 {
        seq: u64,
        ticks: u64,
        kind: u32,
        cpu_id: u32,
        arg0: u64,
        arg1: u64,
    }

    #[repr(C)]
    struct OracleKernelObservabilityV1 {
        version: u32,
        cpu_count: u32,
        trace_capacity: u32,
        trace_count: u32,
        heap_current_bytes: u64,
        heap_high_water_bytes: u64,
        heap_alloc_failures: u64,
        syscall_total: u64,
        syscall_ipc: u64,
        syscall_file: u64,
        syscall_ai: u64,
        syscall_net: u64,
        syscall_other: u64,
        frames_used: u64,
        buddy_free_pages: u64,
        buddy_largest_free_order: u32,
        _pad0: u32,
        sched: [OracleSchedulerStatsV1; MAX_CPUS],
        trace: [OracleTraceEventV1; TRACE_EVENTS],
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    struct ExpectedField {
        name: &'static str,
        ty: &'static str,
        offset: usize,
    }

    const SCHED_FIELDS: [ExpectedField; 10] = [
        ExpectedField {
            name: "cpu_id",
            ty: "u32",
            offset: 0,
        },
        ExpectedField {
            name: "_pad0",
            ty: "u32",
            offset: 4,
        },
        ExpectedField {
            name: "context_switches",
            ty: "u64",
            offset: 8,
        },
        ExpectedField {
            name: "dispatches",
            ty: "u64",
            offset: 16,
        },
        ExpectedField {
            name: "ready_queue_depth",
            ty: "u64",
            offset: 24,
        },
        ExpectedField {
            name: "run_ticks",
            ty: "u64",
            offset: 32,
        },
        ExpectedField {
            name: "idle_ticks",
            ty: "u64",
            offset: 40,
        },
        ExpectedField {
            name: "timer_preemptions",
            ty: "u64",
            offset: 48,
        },
        ExpectedField {
            name: "voluntary_yields",
            ty: "u64",
            offset: 56,
        },
        ExpectedField {
            name: "ready_queue_high_water",
            ty: "u64",
            offset: 64,
        },
    ];

    const TRACE_FIELDS: [ExpectedField; 6] = [
        ExpectedField {
            name: "seq",
            ty: "u64",
            offset: 0,
        },
        ExpectedField {
            name: "ticks",
            ty: "u64",
            offset: 8,
        },
        ExpectedField {
            name: "kind",
            ty: "u32",
            offset: 16,
        },
        ExpectedField {
            name: "cpu_id",
            ty: "u32",
            offset: 20,
        },
        ExpectedField {
            name: "arg0",
            ty: "u64",
            offset: 24,
        },
        ExpectedField {
            name: "arg1",
            ty: "u64",
            offset: 32,
        },
    ];

    const OBSERVABILITY_FIELDS: [ExpectedField; 19] = [
        ExpectedField {
            name: "version",
            ty: "u32",
            offset: 0,
        },
        ExpectedField {
            name: "cpu_count",
            ty: "u32",
            offset: 4,
        },
        ExpectedField {
            name: "trace_capacity",
            ty: "u32",
            offset: 8,
        },
        ExpectedField {
            name: "trace_count",
            ty: "u32",
            offset: 12,
        },
        ExpectedField {
            name: "heap_current_bytes",
            ty: "u64",
            offset: 16,
        },
        ExpectedField {
            name: "heap_high_water_bytes",
            ty: "u64",
            offset: 24,
        },
        ExpectedField {
            name: "heap_alloc_failures",
            ty: "u64",
            offset: 32,
        },
        ExpectedField {
            name: "syscall_total",
            ty: "u64",
            offset: 40,
        },
        ExpectedField {
            name: "syscall_ipc",
            ty: "u64",
            offset: 48,
        },
        ExpectedField {
            name: "syscall_file",
            ty: "u64",
            offset: 56,
        },
        ExpectedField {
            name: "syscall_ai",
            ty: "u64",
            offset: 64,
        },
        ExpectedField {
            name: "syscall_net",
            ty: "u64",
            offset: 72,
        },
        ExpectedField {
            name: "syscall_other",
            ty: "u64",
            offset: 80,
        },
        ExpectedField {
            name: "frames_used",
            ty: "u64",
            offset: 88,
        },
        ExpectedField {
            name: "buddy_free_pages",
            ty: "u64",
            offset: 96,
        },
        ExpectedField {
            name: "buddy_largest_free_order",
            ty: "u32",
            offset: 104,
        },
        ExpectedField {
            name: "_pad0",
            ty: "u32",
            offset: 108,
        },
        ExpectedField {
            name: "sched",
            ty: "[SchedulerStatsV1; KERNEL_OBSERVABILITY_MAX_CPUS]",
            offset: OBSERVABILITY_SCHED_OFFSET,
        },
        ExpectedField {
            name: "trace",
            ty: "[TraceEventV1; KERNEL_OBSERVABILITY_TRACE_EVENTS]",
            offset: OBSERVABILITY_TRACE_OFFSET,
        },
    ];

    struct ByteCheck {
        offset: usize,
        bytes: &'static [u8],
        label: &'static str,
    }

    const SCHED0: usize = OBSERVABILITY_SCHED_OFFSET;
    const SCHED1: usize = OBSERVABILITY_SCHED_OFFSET + SCHEDULER_STATS_SIZE;
    const TRACE0: usize = OBSERVABILITY_TRACE_OFFSET;
    const TRACE1: usize = OBSERVABILITY_TRACE_OFFSET + TRACE_EVENT_SIZE;

    const BYTE_CHECKS: &[ByteCheck] = &[
        ByteCheck {
            offset: 0,
            bytes: &[0x03, 0x00, 0x00, 0x00],
            label: "version",
        },
        ByteCheck {
            offset: 4,
            bytes: &[0x02, 0x00, 0x00, 0x00],
            label: "cpu_count",
        },
        ByteCheck {
            offset: 8,
            bytes: &[0x10, 0x00, 0x00, 0x00],
            label: "trace_capacity",
        },
        ByteCheck {
            offset: 12,
            bytes: &[0x05, 0x00, 0x00, 0x00],
            label: "trace_count",
        },
        ByteCheck {
            offset: 16,
            bytes: &[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
            label: "heap_current_bytes",
        },
        ByteCheck {
            offset: 24,
            bytes: &[0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21],
            label: "heap_high_water_bytes",
        },
        ByteCheck {
            offset: 32,
            bytes: &[0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31],
            label: "heap_alloc_failures",
        },
        ByteCheck {
            offset: 40,
            bytes: &[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41],
            label: "syscall_total",
        },
        ByteCheck {
            offset: 48,
            bytes: &[0x58, 0x57, 0x56, 0x55, 0x54, 0x53, 0x52, 0x51],
            label: "syscall_ipc",
        },
        ByteCheck {
            offset: 56,
            bytes: &[0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61],
            label: "syscall_file",
        },
        ByteCheck {
            offset: 64,
            bytes: &[0x78, 0x77, 0x76, 0x75, 0x74, 0x73, 0x72, 0x71],
            label: "syscall_ai",
        },
        ByteCheck {
            offset: 72,
            bytes: &[0x88, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82, 0x81],
            label: "syscall_net",
        },
        ByteCheck {
            offset: 80,
            bytes: &[0x98, 0x97, 0x96, 0x95, 0x94, 0x93, 0x92, 0x91],
            label: "syscall_other",
        },
        ByteCheck {
            offset: 88,
            bytes: &[0xa8, 0xa7, 0xa6, 0xa5, 0xa4, 0xa3, 0xa2, 0xa1],
            label: "frames_used",
        },
        ByteCheck {
            offset: 96,
            bytes: &[0xb8, 0xb7, 0xb6, 0xb5, 0xb4, 0xb3, 0xb2, 0xb1],
            label: "buddy_free_pages",
        },
        ByteCheck {
            offset: 104,
            bytes: &[0xc4, 0xc3, 0xc2, 0xc1],
            label: "buddy_largest_free_order",
        },
        ByteCheck {
            offset: 108,
            bytes: &[0xd4, 0xd3, 0xd2, 0xd1],
            label: "_pad0",
        },
        ByteCheck {
            offset: SCHED0,
            bytes: &[0x44, 0x43, 0x42, 0x41],
            label: "sched[0].cpu_id",
        },
        ByteCheck {
            offset: SCHED0 + 4,
            bytes: &[0x54, 0x53, 0x52, 0x51],
            label: "sched[0]._pad0",
        },
        ByteCheck {
            offset: SCHED0 + 8,
            bytes: &[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
            label: "sched[0].context_switches",
        },
        ByteCheck {
            offset: SCHED0 + 16,
            bytes: &[0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21],
            label: "sched[0].dispatches",
        },
        ByteCheck {
            offset: SCHED0 + 24,
            bytes: &[0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31],
            label: "sched[0].ready_queue_depth",
        },
        ByteCheck {
            offset: SCHED0 + 32,
            bytes: &[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41],
            label: "sched[0].run_ticks",
        },
        ByteCheck {
            offset: SCHED0 + 40,
            bytes: &[0x58, 0x57, 0x56, 0x55, 0x54, 0x53, 0x52, 0x51],
            label: "sched[0].idle_ticks",
        },
        ByteCheck {
            offset: SCHED0 + 48,
            bytes: &[0x68, 0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61],
            label: "sched[0].timer_preemptions",
        },
        ByteCheck {
            offset: SCHED0 + 56,
            bytes: &[0x78, 0x77, 0x76, 0x75, 0x74, 0x73, 0x72, 0x71],
            label: "sched[0].voluntary_yields",
        },
        ByteCheck {
            offset: SCHED0 + 64,
            bytes: &[0x88, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82, 0x81],
            label: "sched[0].ready_queue_high_water",
        },
        ByteCheck {
            offset: SCHED1,
            bytes: &[0x01, 0x00, 0x00, 0x00],
            label: "sched[1].cpu_id",
        },
        ByteCheck {
            offset: SCHED1 + 64,
            bytes: &[0x98, 0x97, 0x96, 0x95, 0x94, 0x93, 0x92, 0x91],
            label: "sched[1].ready_queue_high_water",
        },
        ByteCheck {
            offset: TRACE0,
            bytes: &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01],
            label: "trace[0].seq",
        },
        ByteCheck {
            offset: TRACE0 + 8,
            bytes: &[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
            label: "trace[0].ticks",
        },
        ByteCheck {
            offset: TRACE0 + 16,
            bytes: &[0x03, 0x00, 0x00, 0x00],
            label: "trace[0].kind",
        },
        ByteCheck {
            offset: TRACE0 + 20,
            bytes: &[0x02, 0x00, 0x00, 0x00],
            label: "trace[0].cpu_id",
        },
        ByteCheck {
            offset: TRACE0 + 24,
            bytes: &[0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21],
            label: "trace[0].arg0",
        },
        ByteCheck {
            offset: TRACE0 + 32,
            bytes: &[0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31],
            label: "trace[0].arg1",
        },
        ByteCheck {
            offset: TRACE1,
            bytes: &[0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            label: "trace[1].seq",
        },
        ByteCheck {
            offset: TRACE1 + 32,
            bytes: &[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41],
            label: "trace[1].arg1",
        },
    ];

    #[test]
    fn source_defs_match_the_independent_oracle() {
        assert_contains(
            LIBNEO_SRC,
            "pub const KERNEL_OBSERVABILITY_V1_VERSION: u32 = 3;",
        );
        assert_contains(
            KERNEL_OBSERVABILITY_SRC,
            "pub const KERNEL_OBSERVABILITY_V1_VERSION: u32 = 3;",
        );
        assert_contains(
            LIBNEO_SRC,
            "pub const KERNEL_OBSERVABILITY_MAX_CPUS: usize = 64;",
        );
        assert_contains(KERNEL_PER_CPU_SRC, "pub const MAX_CPUS: usize = 64;");
        assert_contains(
            KERNEL_OBSERVABILITY_SRC,
            "pub const KERNEL_OBSERVABILITY_MAX_CPUS: usize = crate::arch::x86_64::per_cpu::MAX_CPUS;",
        );
        assert_contains(
            LIBNEO_SRC,
            "pub const KERNEL_OBSERVABILITY_TRACE_EVENTS: usize = 16;",
        );
        assert_contains(
            KERNEL_OBSERVABILITY_SRC,
            "pub const KERNEL_OBSERVABILITY_TRACE_EVENTS: usize = 16;",
        );

        for (label, source) in [
            ("libneo", LIBNEO_SRC),
            ("kernel observability", KERNEL_OBSERVABILITY_SRC),
        ] {
            assert_fields(label, source, "SchedulerStatsV1", &SCHED_FIELDS);
            assert_fields(label, source, "TraceEventV1", &TRACE_FIELDS);
            assert_fields(
                label,
                source,
                "KernelObservabilityV1",
                &OBSERVABILITY_FIELDS,
            );
        }
    }

    #[test]
    fn oracle_struct_offsets_are_the_expected_c_repr_layout() {
        assert_eq!(ABI_VERSION, 3);
        assert_eq!(MAX_CPUS, 64);
        assert_eq!(TRACE_EVENTS, 16);

        assert_eq!(size_of::<OracleSchedulerStatsV1>(), SCHEDULER_STATS_SIZE);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, cpu_id), 0);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, _pad0), 4);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, context_switches), 8);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, dispatches), 16);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, ready_queue_depth), 24);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, run_ticks), 32);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, idle_ticks), 40);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, timer_preemptions), 48);
        assert_eq!(offset_of!(OracleSchedulerStatsV1, voluntary_yields), 56);
        assert_eq!(
            offset_of!(OracleSchedulerStatsV1, ready_queue_high_water),
            64
        );

        assert_eq!(size_of::<OracleTraceEventV1>(), TRACE_EVENT_SIZE);
        assert_eq!(offset_of!(OracleTraceEventV1, seq), 0);
        assert_eq!(offset_of!(OracleTraceEventV1, ticks), 8);
        assert_eq!(offset_of!(OracleTraceEventV1, kind), 16);
        assert_eq!(offset_of!(OracleTraceEventV1, cpu_id), 20);
        assert_eq!(offset_of!(OracleTraceEventV1, arg0), 24);
        assert_eq!(offset_of!(OracleTraceEventV1, arg1), 32);

        assert_eq!(size_of::<OracleKernelObservabilityV1>(), OBSERVABILITY_SIZE);
        assert_eq!(OBSERVABILITY_TRACE_OFFSET, 4720);
        assert_eq!(OBSERVABILITY_SIZE, 5360);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, version), 0);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, cpu_count), 4);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, trace_capacity), 8);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, trace_count), 12);
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, heap_current_bytes),
            16
        );
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, heap_high_water_bytes),
            24
        );
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, heap_alloc_failures),
            32
        );
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_total), 40);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_ipc), 48);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_file), 56);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_ai), 64);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_net), 72);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, syscall_other), 80);
        assert_eq!(offset_of!(OracleKernelObservabilityV1, frames_used), 88);
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, buddy_free_pages),
            96
        );
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, buddy_largest_free_order),
            104
        );
        assert_eq!(offset_of!(OracleKernelObservabilityV1, _pad0), 108);
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, sched),
            OBSERVABILITY_SCHED_OFFSET
        );
        assert_eq!(
            offset_of!(OracleKernelObservabilityV1, trace),
            OBSERVABILITY_TRACE_OFFSET
        );
    }

    #[test]
    fn hand_laid_byte_fixture_pins_offsets_and_array_strides() {
        let fixture = hand_laid_fixture();
        assert_eq!(fixture.len(), OBSERVABILITY_SIZE);

        for check in BYTE_CHECKS {
            let got = &fixture[check.offset..check.offset + check.bytes.len()];
            assert_eq!(
                got, check.bytes,
                "{} at offset 0x{:x}",
                check.label, check.offset
            );
        }

        assert!(fixture[SCHED0 + SCHEDULER_STATS_SIZE - 1] != 0);
        assert!(fixture[TRACE0 + TRACE_EVENT_SIZE - 1] != 0);
        assert_eq!(SCHED1 - SCHED0, SCHEDULER_STATS_SIZE);
        assert_eq!(TRACE1 - TRACE0, TRACE_EVENT_SIZE);
    }

    fn hand_laid_fixture() -> Vec<u8> {
        let mut bytes = vec![0; OBSERVABILITY_SIZE];
        for check in BYTE_CHECKS {
            bytes[check.offset..check.offset + check.bytes.len()].copy_from_slice(check.bytes);
        }
        bytes
    }

    fn assert_contains(haystack: &str, needle: &str) {
        assert!(haystack.contains(needle), "missing source text: {needle}");
    }

    fn assert_fields(label: &str, source: &str, struct_name: &str, expected: &[ExpectedField]) {
        let got = parse_pub_fields(struct_body(source, struct_name));
        let expected_pairs: Vec<(&str, &str)> = expected
            .iter()
            .map(|field| (field.name, field.ty))
            .collect();
        assert_eq!(
            got, expected_pairs,
            "{label} {struct_name} field order/type drifted"
        );

        for field in expected {
            assert!(
                field.offset < OBSERVABILITY_SIZE,
                "{struct_name}.{} has impossible oracle offset {}",
                field.name,
                field.offset
            );
        }
    }

    fn struct_body<'a>(source: &'a str, struct_name: &str) -> &'a str {
        let marker = format!("pub struct {struct_name} {{");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing struct marker: {marker}"))
            + marker.len();
        let tail = &source[start..];
        let end = tail
            .find("\n}")
            .unwrap_or_else(|| panic!("missing struct end: {struct_name}"));
        &tail[..end]
    }

    fn parse_pub_fields(body: &str) -> Vec<(&str, &str)> {
        body.lines()
            .filter_map(|line| {
                let line = line.trim();
                let field = line.strip_prefix("pub ")?;
                let (name, ty) = field.split_once(':')?;
                Some((name.trim(), ty.trim().trim_end_matches(',')))
            })
            .collect()
    }
}
