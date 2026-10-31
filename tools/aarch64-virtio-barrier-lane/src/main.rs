// SPDX-License-Identifier: MPL-2.0
#![no_std]
#![no_main]
#![allow(static_mut_refs)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_bytes, write_volatile};
use core::sync::atomic::{Ordering, fence};

#[path = "../../../kernel/src/drivers/block/virtio_barrier.rs"]
mod virtio_barrier;

const SCENARIO: &str = "phase3_aarch64_virtio_barrier_lane";

const PL011_BASE: usize = 0x0900_0000;
const PL011_DR: usize = PL011_BASE;
const PL011_FR: usize = PL011_BASE + 0x18;
const PL011_FR_TXFF: u32 = 1 << 5;

const VIRTIO_MMIO_BASE_START: usize = 0x0a00_0000;
const VIRTIO_MMIO_STRIDE: usize = 0x200;
const VIRTIO_MMIO_SLOTS: usize = 32;

const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
const VIRTIO_MMIO_VERSION: usize = 0x004;
const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;
const VIRTIO_MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;
const VIRTIO_MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;
const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;
const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
const VIRTIO_MMIO_STATUS: usize = 0x070;
const VIRTIO_MMIO_QUEUE_DESC_LOW: usize = 0x080;
const VIRTIO_MMIO_QUEUE_DRIVER_LOW: usize = 0x090;
const VIRTIO_MMIO_QUEUE_DEVICE_LOW: usize = 0x0a0;
const VIRTIO_MMIO_CONFIG: usize = 0x100;

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const VIRTIO_DEVICE_BLOCK: u32 = 2;
const VIRTIO_RING_F_EVENT_IDX: u64 = 1 << 29;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const STATUS_FEATURES_OK: u32 = 8;
const STATUS_FAILED: u32 = 128;

const QUEUE_SIZE: u16 = 8;
const SECTOR_SIZE: usize = 512;
const TEST_SECTOR: u64 = 8;
const POLL_LIMIT: usize = 50_000_000;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const VIRTIO_BLK_S_OK: u8 = 0;

static mut PUBLISH_DESCRIPTOR_BARRIERS: u32 = 0;
static mut NOTIFICATION_CHECK_BARRIERS: u32 = 0;
static mut NOTIFICATION_DECISIONS: u32 = 0;
static mut NOTIFICATIONS_SENT: u32 = 0;

global_asm!(
    r#"
    .section .text.boot
    .global _start
_start:
    ldr x0, =__stack_top
    mov sp, x0

    ldr x0, =__bss_start
    ldr x1, =__bss_end
1:
    cmp x0, x1
    b.hs 2f
    str xzr, [x0], #8
    b 1b
2:
    bl rust_main
3:
    wfe
    b 3b
"#
);

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AvailRing {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE as usize],
    used_event: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UsedRing {
    flags: u16,
    idx: u16,
    ring: [UsedElem; QUEUE_SIZE as usize],
    avail_event: u16,
}

#[repr(C, align(4096))]
struct QueueMemory {
    desc: [Descriptor; QUEUE_SIZE as usize],
    avail: AvailRing,
    used: UsedRing,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BlkReqHeader {
    request_type: u32,
    reserved: u32,
    sector: u64,
}

#[repr(C, align(16))]
struct AlignedHeader(BlkReqHeader);

#[repr(C, align(4096))]
struct AlignedBlock([u8; SECTOR_SIZE]);

const EMPTY_DESC: Descriptor = Descriptor {
    addr: 0,
    len: 0,
    flags: 0,
    next: 0,
};

const EMPTY_USED: UsedElem = UsedElem { id: 0, len: 0 };

static mut QUEUE: QueueMemory = QueueMemory {
    desc: [EMPTY_DESC; QUEUE_SIZE as usize],
    avail: AvailRing {
        flags: 0,
        idx: 0,
        ring: [0; QUEUE_SIZE as usize],
        used_event: 0,
    },
    used: UsedRing {
        flags: 0,
        idx: 0,
        ring: [EMPTY_USED; QUEUE_SIZE as usize],
        avail_event: 0,
    },
};

static mut HEADER: AlignedHeader = AlignedHeader(BlkReqHeader {
    request_type: 0,
    reserved: 0,
    sector: 0,
});
static mut STATUS_BYTE: u8 = 0xff;
static mut WRITE_BLOCK: AlignedBlock = AlignedBlock([0; SECTOR_SIZE]);
static mut READ_BLOCK: AlignedBlock = AlignedBlock([0; SECTOR_SIZE]);

#[unsafe(no_mangle)]
extern "C" fn rust_main() -> ! {
    serial_line(
        "NEO_TEST/1 event=BEGIN name=phase3_aarch64_virtio_barrier_lane phase=dispatch runtime=aarch64",
    );

    unsafe {
        zero_queue();
        fill_write_pattern();
        clear_read_pattern();
    }
    serial_line("[ aarch64 ] harness memory initialized");

    let base = match find_virtio_blk() {
        Some(base) => base,
        None => fail("virtio_mmio_blk_not_found"),
    };

    let capacity = read_config_capacity(base);
    serial_str("[ aarch64 ] virtio-blk capacity=");
    serial_dec(capacity);
    serial_str(" sectors\n");
    if capacity <= TEST_SECTOR {
        fail("virtio_blk_capacity_too_small");
    }

    let features = negotiate_features(base);
    setup_queue(base);

    submit_request(
        base,
        VIRTIO_BLK_T_OUT,
        TEST_SECTOR,
        buffer_addr(true),
        false,
        0,
    );
    wait_for_used_idx(1, "write_completion_timeout");
    if unsafe { read_volatile(addr_of!(STATUS_BYTE)) } != VIRTIO_BLK_S_OK {
        fail("write_status_not_ok");
    }

    submit_request(
        base,
        VIRTIO_BLK_T_IN,
        TEST_SECTOR,
        buffer_addr(false),
        true,
        1,
    );
    wait_for_used_idx(2, "read_completion_timeout");
    if unsafe { read_volatile(addr_of!(STATUS_BYTE)) } != VIRTIO_BLK_S_OK {
        fail("read_status_not_ok");
    }
    if !unsafe { blocks_match() } {
        fail("readback_data_mismatch");
    }

    serial_str("NEO_TEST/1 event=PASS name=");
    serial_str(SCENARIO);
    serial_str(
        " runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x",
    );
    serial_hex32(features as u32);
    serial_str(" feature_high=0x");
    serial_hex32((features >> 32) as u32);
    serial_str(" sector=");
    serial_dec(TEST_SECTOR);
    serial_str(" block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=");
    serial_dec(unsafe { PUBLISH_DESCRIPTOR_BARRIERS as u64 });
    serial_str(" before_notification_check=");
    serial_dec(unsafe { NOTIFICATION_CHECK_BARRIERS as u64 });
    serial_str(" notification_decisions=");
    serial_dec(unsafe { NOTIFICATION_DECISIONS as u64 });
    serial_str(" notifications_sent=");
    serial_dec(unsafe { NOTIFICATIONS_SENT as u64 });
    serial_str(" compile_only=false sentinel=runtime_qemu_aarch64\n");
    semihost_exit(0);
}

fn find_virtio_blk() -> Option<usize> {
    let mut slot = 0;
    while slot < VIRTIO_MMIO_SLOTS {
        let base = VIRTIO_MMIO_BASE_START + slot * VIRTIO_MMIO_STRIDE;
        let magic = mmio_read32(base + VIRTIO_MMIO_MAGIC_VALUE);
        let version = mmio_read32(base + VIRTIO_MMIO_VERSION);
        let device = mmio_read32(base + VIRTIO_MMIO_DEVICE_ID);
        if magic == VIRTIO_MAGIC && version >= 2 && device == VIRTIO_DEVICE_BLOCK {
            serial_str("[ aarch64 ] virtio-mmio blk base=0x");
            serial_hex64(base as u64);
            serial_str(" version=");
            serial_dec(version as u64);
            serial_str("\n");
            return Some(base);
        }
        slot += 1;
    }
    None
}

fn negotiate_features(base: usize) -> u64 {
    mmio_write32(base + VIRTIO_MMIO_STATUS, 0);
    mmio_write32(base + VIRTIO_MMIO_STATUS, STATUS_ACKNOWLEDGE);
    mmio_write32(
        base + VIRTIO_MMIO_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER,
    );

    mmio_write32(base + VIRTIO_MMIO_DEVICE_FEATURES_SEL, 0);
    let features_low = mmio_read32(base + VIRTIO_MMIO_DEVICE_FEATURES) as u64;
    mmio_write32(base + VIRTIO_MMIO_DEVICE_FEATURES_SEL, 1);
    let features_high = mmio_read32(base + VIRTIO_MMIO_DEVICE_FEATURES) as u64;
    let features = features_low | (features_high << 32);

    if features & VIRTIO_F_VERSION_1 == 0 {
        fail("virtio_version_1_not_offered");
    }
    if features & VIRTIO_RING_F_EVENT_IDX == 0 {
        fail("virtio_event_idx_not_offered");
    }

    mmio_write32(base + VIRTIO_MMIO_DRIVER_FEATURES_SEL, 0);
    mmio_write32(
        base + VIRTIO_MMIO_DRIVER_FEATURES,
        (VIRTIO_RING_F_EVENT_IDX & 0xffff_ffff) as u32,
    );
    mmio_write32(base + VIRTIO_MMIO_DRIVER_FEATURES_SEL, 1);
    mmio_write32(
        base + VIRTIO_MMIO_DRIVER_FEATURES,
        (VIRTIO_F_VERSION_1 >> 32) as u32,
    );

    mmio_write32(
        base + VIRTIO_MMIO_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
    );
    let status = mmio_read32(base + VIRTIO_MMIO_STATUS);
    if status & STATUS_FEATURES_OK == 0 {
        fail("virtio_features_ok_rejected");
    }

    serial_str("[ aarch64 ] virtio-mmio features low=0x");
    serial_hex32(features_low as u32);
    serial_str(" high=0x");
    serial_hex32(features_high as u32);
    serial_str(" negotiated=VERSION_1,EVENT_IDX\n");
    features
}

fn setup_queue(base: usize) {
    mmio_write32(base + VIRTIO_MMIO_QUEUE_SEL, 0);
    let max = mmio_read32(base + VIRTIO_MMIO_QUEUE_NUM_MAX);
    if max < QUEUE_SIZE as u32 {
        fail("virtqueue_too_small");
    }
    if mmio_read32(base + VIRTIO_MMIO_QUEUE_READY) != 0 {
        fail("virtqueue_unexpected_ready");
    }

    let desc = unsafe { addr_of_mut!(QUEUE.desc) as *mut _ as u64 };
    let driver = unsafe { addr_of_mut!(QUEUE.avail) as *mut _ as u64 };
    let device = unsafe { addr_of_mut!(QUEUE.used) as *mut _ as u64 };

    mmio_write32(base + VIRTIO_MMIO_QUEUE_NUM, QUEUE_SIZE as u32);
    write_addr64(base + VIRTIO_MMIO_QUEUE_DESC_LOW, desc);
    write_addr64(base + VIRTIO_MMIO_QUEUE_DRIVER_LOW, driver);
    write_addr64(base + VIRTIO_MMIO_QUEUE_DEVICE_LOW, device);
    mmio_write32(base + VIRTIO_MMIO_QUEUE_READY, 1);

    mmio_write32(
        base + VIRTIO_MMIO_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
    );

    serial_str("[ aarch64 ] virtqueue ready size=");
    serial_dec(QUEUE_SIZE as u64);
    serial_str(" desc=0x");
    serial_hex64(desc);
    serial_str(" avail=0x");
    serial_hex64(driver);
    serial_str(" used=0x");
    serial_hex64(device);
    serial_str("\n");
}

fn submit_request(
    base: usize,
    request_type: u32,
    sector: u64,
    data_addr: u64,
    device_writes_data: bool,
    avail_slot: u16,
) {
    unsafe {
        write_request_header(request_type, sector);
        write_volatile(addr_of_mut!(STATUS_BYTE), 0xff);

        write_descriptor(0, addr_of!(HEADER) as u64, 16, VIRTQ_DESC_F_NEXT, 1);
        write_descriptor(
            1,
            data_addr,
            SECTOR_SIZE as u32,
            if device_writes_data {
                VIRTQ_DESC_F_WRITE | VIRTQ_DESC_F_NEXT
            } else {
                VIRTQ_DESC_F_NEXT
            },
            2,
        );
        write_descriptor(2, addr_of!(STATUS_BYTE) as u64, 1, VIRTQ_DESC_F_WRITE, 0);

        let old_idx = read_volatile(addr_of!(QUEUE.avail.idx));
        write_volatile(
            addr_of_mut!(QUEUE.avail.ring[avail_slot as usize]),
            0u16.to_le(),
        );
        publish_descriptors();
        let new_idx = old_idx.wrapping_add(1);
        write_volatile(addr_of_mut!(QUEUE.avail.idx), new_idx.to_le());
        before_notification_check();
        let event = u16::from_le(read_volatile(addr_of!(QUEUE.used.avail_event)));
        NOTIFICATION_DECISIONS = NOTIFICATION_DECISIONS.wrapping_add(1);
        let notify = vring_need_event(event, new_idx, old_idx);
        serial_str("[ aarch64 ] submit type=");
        serial_dec(request_type as u64);
        serial_str(" old_idx=");
        serial_dec(old_idx as u64);
        serial_str(" new_idx=");
        serial_dec(new_idx as u64);
        serial_str(" avail_event=");
        serial_dec(event as u64);
        serial_str(" notify=");
        serial_bool(notify);
        serial_str("\n");
        if notify {
            NOTIFICATIONS_SENT = NOTIFICATIONS_SENT.wrapping_add(1);
            mmio_write32(base + VIRTIO_MMIO_QUEUE_NOTIFY, 0);
        }
    }
}

fn wait_for_used_idx(expected: u16, reason: &'static str) {
    let mut spins = 0usize;
    while spins < POLL_LIMIT {
        let idx = unsafe { u16::from_le(read_volatile(addr_of!(QUEUE.used.idx))) };
        if idx == expected {
            fence(Ordering::Acquire);
            return;
        }
        spins += 1;
    }
    fail(reason);
}

fn publish_descriptors() {
    unsafe {
        PUBLISH_DESCRIPTOR_BARRIERS = PUBLISH_DESCRIPTOR_BARRIERS.wrapping_add(1);
    }
    virtio_barrier::publish_descriptors();
}

fn before_notification_check() {
    unsafe {
        NOTIFICATION_CHECK_BARRIERS = NOTIFICATION_CHECK_BARRIERS.wrapping_add(1);
    }
    virtio_barrier::before_notification_check();
}

fn vring_need_event(event_idx: u16, new_idx: u16, old_idx: u16) -> bool {
    new_idx.wrapping_sub(event_idx).wrapping_sub(1) < new_idx.wrapping_sub(old_idx)
}

unsafe fn zero_queue() {
    write_bytes(
        addr_of_mut!(QUEUE) as *mut u8,
        0,
        core::mem::size_of::<QueueMemory>(),
    );
}

unsafe fn fill_write_pattern() {
    let block = addr_of_mut!(WRITE_BLOCK.0) as *mut u8;
    let mut i = 0usize;
    while i < SECTOR_SIZE {
        write_volatile(block.add(i), (i as u8).wrapping_mul(37).wrapping_add(0x5a));
        i += 1;
    }
}

unsafe fn clear_read_pattern() {
    let block = addr_of_mut!(READ_BLOCK.0) as *mut u8;
    let mut i = 0usize;
    while i < SECTOR_SIZE {
        write_volatile(block.add(i), 0);
        i += 1;
    }
}

unsafe fn blocks_match() -> bool {
    let read_block = addr_of!(READ_BLOCK.0) as *const u8;
    let write_block = addr_of!(WRITE_BLOCK.0) as *const u8;
    let mut i = 0usize;
    while i < SECTOR_SIZE {
        if read_volatile(read_block.add(i)) != read_volatile(write_block.add(i)) {
            return false;
        }
        i += 1;
    }
    true
}

fn buffer_addr(write: bool) -> u64 {
    unsafe {
        if write {
            addr_of!(WRITE_BLOCK.0) as u64
        } else {
            addr_of!(READ_BLOCK.0) as u64
        }
    }
}

unsafe fn write_request_header(request_type: u32, sector: u64) {
    write_volatile(addr_of_mut!(HEADER.0.request_type), request_type.to_le());
    write_volatile(addr_of_mut!(HEADER.0.reserved), 0);
    write_volatile(addr_of_mut!(HEADER.0.sector), sector.to_le());
}

unsafe fn write_descriptor(index: usize, addr: u64, len: u32, flags: u16, next: u16) {
    write_volatile(addr_of_mut!(QUEUE.desc[index].addr), addr.to_le());
    write_volatile(addr_of_mut!(QUEUE.desc[index].len), len.to_le());
    write_volatile(addr_of_mut!(QUEUE.desc[index].flags), flags.to_le());
    write_volatile(addr_of_mut!(QUEUE.desc[index].next), next.to_le());
}

fn read_config_capacity(base: usize) -> u64 {
    let low = mmio_read32(base + VIRTIO_MMIO_CONFIG) as u64;
    let high = mmio_read32(base + VIRTIO_MMIO_CONFIG + 4) as u64;
    low | (high << 32)
}

fn write_addr64(offset_low: usize, addr: u64) {
    mmio_write32(offset_low, addr as u32);
    mmio_write32(offset_low + 4, (addr >> 32) as u32);
}

fn mmio_read32(addr: usize) -> u32 {
    unsafe { u32::from_le(read_volatile(addr as *const u32)) }
}

fn mmio_write32(addr: usize, value: u32) {
    unsafe { write_volatile(addr as *mut u32, value.to_le()) }
}

fn fail(reason: &'static str) -> ! {
    mmio_write32(VIRTIO_MMIO_BASE_START + VIRTIO_MMIO_STATUS, STATUS_FAILED);
    serial_str("NEO_TEST/1 event=FAIL name=");
    serial_str(SCENARIO);
    serial_str(" reason=");
    serial_str(reason);
    serial_str(" runtime=aarch64 compile_only=false\n");
    semihost_exit(1);
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    fail("panic");
}

fn serial_line(s: &str) {
    serial_str(s);
    serial_str("\n");
}

fn serial_str(s: &str) {
    for byte in s.bytes() {
        serial_byte(byte);
    }
}

fn serial_byte(byte: u8) {
    unsafe {
        while read_volatile(PL011_FR as *const u32) & PL011_FR_TXFF != 0 {}
        write_volatile(PL011_DR as *mut u32, byte as u32);
    }
}

fn serial_bool(value: bool) {
    serial_str(if value { "true" } else { "false" });
}

fn serial_dec(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        serial_byte(b'0');
        return;
    }
    while value != 0 {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    while i < buf.len() {
        serial_byte(buf[i]);
        i += 1;
    }
}

fn serial_hex32(value: u32) {
    serial_hex_fixed(value as u64, 8);
}

fn serial_hex64(value: u64) {
    serial_hex_fixed(value, 16);
}

fn serial_hex_fixed(value: u64, digits: usize) {
    let mut shift = (digits - 1) * 4;
    loop {
        let nibble = ((value >> shift) & 0xf) as u8;
        serial_byte(if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + (nibble - 10)
        });
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
}

#[repr(C)]
struct SemihostExit {
    reason: u64,
    status: u64,
}

fn semihost_exit(status: u64) -> ! {
    let args = SemihostExit {
        reason: 0x20026,
        status,
    };
    unsafe {
        asm!(
            "hlt #0xf000",
            in("x0") 0x20_u64,
            in("x1") &args as *const SemihostExit as u64,
            options(noreturn)
        );
    }
}
