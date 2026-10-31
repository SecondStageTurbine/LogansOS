// SPDX-License-Identifier: MPL-2.0
use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

const MAGIC: &[u8; 11] = b"NEO_LOG_v1\0";
const HEADER_SIZE: usize = 4096;
const DEFAULT_OFFSET: u64 = 0x1000_0000;
const BUILD_ID_LEN: usize = 32;
const SCENARIO_NAME_LEN: usize = 64;
const OFFSET_REGION_SIZE: usize = 0x010;
const OFFSET_WRITE_CURSOR: usize = 0x018;
const OFFSET_BUILD_ID: usize = 0x020;
const OFFSET_SCENARIO_NAME: usize = 0x040;
const OFFSET_START_TIMESTAMP_NS: usize = 0x080;
const OFFSET_HEADER_VERSION: usize = 0x088;
const OFFSET_DEGRADATION_REASON: usize = 0x08c;
const OFFSET_DEGRADATION_OP: usize = 0x090;
const OFFSET_DEGRADATION_LBA: usize = 0x098;
const OFFSET_DEGRADATION_CURSOR: usize = 0x0a0;

fn main() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1))?;
    let mut device = File::open(&args.device)
        .map_err(|e| format!("open device {}: {e}", args.device.display()))?;

    let mut header = [0u8; HEADER_SIZE];
    device
        .seek(SeekFrom::Start(args.offset))
        .map_err(|e| format!("seek device {}: {e}", args.device.display()))?;
    device
        .read_exact(&mut header)
        .map_err(|e| format!("read log header: {e}"))?;

    let header = parse_header(&header)?;
    let payload_limit = header
        .region_size
        .checked_sub(HEADER_SIZE as u64)
        .ok_or_else(|| "region size is smaller than header".to_string())?;
    if header.write_cursor > payload_limit {
        return Err(format!(
            "header write_cursor {} exceeds payload capacity {}",
            header.write_cursor, payload_limit
        ));
    }

    let mut payload = vec![0u8; header.write_cursor as usize];
    device
        .seek(SeekFrom::Start(args.offset + HEADER_SIZE as u64))
        .map_err(|e| format!("seek log payload: {e}"))?;
    device
        .read_exact(&mut payload)
        .map_err(|e| format!("read log payload: {e}"))?;

    let mut output = File::create(&args.output)
        .map_err(|e| format!("create output {}: {e}", args.output.display()))?;
    output
        .write_all(&payload)
        .map_err(|e| format!("write output {}: {e}", args.output.display()))?;

    println!("Found NEO_LOG_v1 header at offset 0x{:x}", args.offset);
    println!("Scenario: {}", header.scenario_name);
    println!("Start timestamp ns: {}", header.start_timestamp_ns);
    println!("Captured bytes: {}", header.write_cursor);
    if header.degradation.reason != 0 {
        println!("Degradation detected:");
        println!(
            "  reason: {}",
            degradation_reason_name(header.degradation.reason)
        );
        println!("  op:     {}", degradation_op_name(header.degradation.op));
        println!("  lba:    {}", header.degradation.lba);
        println!("  cursor: {}", header.degradation.cursor);
    }
    println!("Wrote {}", args.output.display());
    Ok(())
}

struct Args {
    device: PathBuf,
    output: PathBuf,
    offset: u64,
}

impl Args {
    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut device = None;
        let mut output = None;
        let mut offset = DEFAULT_OFFSET;
        let mut iter = args.into_iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--device" => {
                    device = Some(PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--device requires a path".to_string())?,
                    ));
                }
                "--output" => {
                    output = Some(PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--output requires a path".to_string())?,
                    ));
                }
                "--offset" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--offset requires a byte offset".to_string())?;
                    offset = parse_u64(&value)?;
                }
                "-h" | "--help" => return Err(usage()),
                other => return Err(format!("unknown argument `{other}`\n{}", usage())),
            }
        }

        Ok(Self {
            device: device.ok_or_else(|| format!("missing --device\n{}", usage()))?,
            output: output.ok_or_else(|| format!("missing --output\n{}", usage()))?,
            offset,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Header {
    region_size: u64,
    write_cursor: u64,
    build_id: [u8; BUILD_ID_LEN],
    scenario_name: String,
    start_timestamp_ns: u64,
    degradation: Degradation,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Degradation {
    reason: u32,
    op: u32,
    lba: u64,
    cursor: u64,
}

fn parse_header(bytes: &[u8; HEADER_SIZE]) -> Result<Header, String> {
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err("NEO_LOG_v1 magic not found at requested offset".to_string());
    }

    let region_size = read_u64(bytes, OFFSET_REGION_SIZE);
    let write_cursor = read_u64(bytes, OFFSET_WRITE_CURSOR);
    let mut build_id = [0u8; BUILD_ID_LEN];
    build_id.copy_from_slice(&bytes[OFFSET_BUILD_ID..OFFSET_BUILD_ID + BUILD_ID_LEN]);
    if build_id.iter().all(|b| *b == 0) {
        return Err("build_id field is zeroed".to_string());
    }

    let scenario_name =
        nul_trimmed_ascii(&bytes[OFFSET_SCENARIO_NAME..OFFSET_SCENARIO_NAME + SCENARIO_NAME_LEN]);
    let start_timestamp_ns = read_u64(bytes, OFFSET_START_TIMESTAMP_NS);
    let _header_version = read_u32(bytes, OFFSET_HEADER_VERSION);
    let degradation = Degradation {
        reason: read_u32(bytes, OFFSET_DEGRADATION_REASON),
        op: read_u32(bytes, OFFSET_DEGRADATION_OP),
        lba: read_u64(bytes, OFFSET_DEGRADATION_LBA),
        cursor: read_u64(bytes, OFFSET_DEGRADATION_CURSOR),
    };

    Ok(Header {
        region_size,
        write_cursor,
        build_id,
        scenario_name,
        start_timestamp_ns,
        degradation,
    })
}

#[cfg(test)]
fn serialize_header(
    out: &mut [u8; HEADER_SIZE],
    region_size: u64,
    write_cursor: u64,
    build_id: [u8; BUILD_ID_LEN],
    scenario_name: &[u8],
    start_timestamp_ns: u64,
    degradation: Degradation,
) {
    out.fill(0);
    out[..MAGIC.len()].copy_from_slice(MAGIC);
    out[OFFSET_REGION_SIZE..OFFSET_REGION_SIZE + 8].copy_from_slice(&region_size.to_le_bytes());
    out[OFFSET_WRITE_CURSOR..OFFSET_WRITE_CURSOR + 8].copy_from_slice(&write_cursor.to_le_bytes());
    out[OFFSET_BUILD_ID..OFFSET_BUILD_ID + BUILD_ID_LEN].copy_from_slice(&build_id);
    let take = scenario_name.len().min(SCENARIO_NAME_LEN);
    out[OFFSET_SCENARIO_NAME..OFFSET_SCENARIO_NAME + take].copy_from_slice(&scenario_name[..take]);
    out[OFFSET_START_TIMESTAMP_NS..OFFSET_START_TIMESTAMP_NS + 8]
        .copy_from_slice(&start_timestamp_ns.to_le_bytes());
    out[OFFSET_HEADER_VERSION..OFFSET_HEADER_VERSION + 4].copy_from_slice(&2u32.to_le_bytes());
    out[OFFSET_DEGRADATION_REASON..OFFSET_DEGRADATION_REASON + 4]
        .copy_from_slice(&degradation.reason.to_le_bytes());
    out[OFFSET_DEGRADATION_OP..OFFSET_DEGRADATION_OP + 4]
        .copy_from_slice(&degradation.op.to_le_bytes());
    out[OFFSET_DEGRADATION_LBA..OFFSET_DEGRADATION_LBA + 8]
        .copy_from_slice(&degradation.lba.to_le_bytes());
    out[OFFSET_DEGRADATION_CURSOR..OFFSET_DEGRADATION_CURSOR + 8]
        .copy_from_slice(&degradation.cursor.to_le_bytes());
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn degradation_reason_name(reason: u32) -> String {
    match reason {
        1 => "Io".to_string(),
        2 => "InvalidRequest".to_string(),
        3 => "Unsupported".to_string(),
        4 => "Other".to_string(),
        other => format!("Unknown({other})"),
    }
}

fn degradation_op_name(op: u32) -> String {
    match op {
        0 => "idle".to_string(),
        1 => "read_block_for_rmw".to_string(),
        2 => "write_block_payload".to_string(),
        3 => "write_block_header_rewrite".to_string(),
        4 => "device_flush_after_write".to_string(),
        other => format!("unknown({other})"),
    }
}

fn nul_trimmed_ascii(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes[..end]
        .iter()
        .map(|b| {
            if b.is_ascii() && !b.is_ascii_control() {
                *b as char
            } else {
                '?'
            }
        })
        .collect()
}

fn parse_u64(value: &str) -> Result<u64, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|e| format!("parse offset `{value}`: {e}"))
    } else {
        value
            .parse::<u64>()
            .map_err(|e| format!("parse offset `{value}`: {e}"))
    }
}

fn usage() -> String {
    "usage: neo-extract-log --device PATH --output PATH [--offset 0x10000000]".to_string()
}

#[cfg(test)]
impl Degradation {
    fn none() -> Self {
        Self {
            reason: 0,
            op: 0,
            lba: 0,
            cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips() {
        let mut header = [0u8; HEADER_SIZE];
        let mut build_id = [0u8; BUILD_ID_LEN];
        build_id[..8].copy_from_slice(b"deadbeef");

        serialize_header(
            &mut header,
            16 * 1024 * 1024,
            184_224,
            build_id,
            b"phase3_ahci_neofs_mount_and_readwrite",
            1234,
            Degradation::none(),
        );

        let parsed = parse_header(&header).expect("valid header");
        assert_eq!(parsed.region_size, 16 * 1024 * 1024);
        assert_eq!(parsed.write_cursor, 184_224);
        assert_eq!(parsed.build_id, build_id);
        assert_eq!(
            parsed.scenario_name,
            "phase3_ahci_neofs_mount_and_readwrite"
        );
        assert_eq!(parsed.start_timestamp_ns, 1234);
        assert_eq!(read_u32(&header, OFFSET_HEADER_VERSION), 2);
        assert_eq!(parsed.degradation, Degradation::none());
    }

    #[test]
    fn degraded_header_round_trips() {
        let mut header = [0u8; HEADER_SIZE];
        let mut build_id = [0u8; BUILD_ID_LEN];
        build_id[..8].copy_from_slice(b"deadbeef");
        let degradation = Degradation {
            reason: 2,
            op: 1,
            lba: 524_296,
            cursor: 128,
        };

        serialize_header(
            &mut header,
            16 * 1024 * 1024,
            128,
            build_id,
            b"phase3_ahci_neofs_mount_and_readwrite",
            0,
            degradation,
        );

        let parsed = parse_header(&header).expect("valid header");
        assert_eq!(parsed.degradation, degradation);
        assert_eq!(
            degradation_reason_name(parsed.degradation.reason),
            "InvalidRequest"
        );
        assert_eq!(
            degradation_op_name(parsed.degradation.op),
            "read_block_for_rmw"
        );
    }

    #[test]
    fn invalid_magic_is_rejected() {
        let mut header = [0u8; HEADER_SIZE];
        let mut build_id = [0u8; BUILD_ID_LEN];
        build_id[0] = b'x';
        serialize_header(
            &mut header,
            4096,
            0,
            build_id,
            b"scenario",
            0,
            Degradation::none(),
        );
        header[0] = b'X';
        assert!(parse_header(&header).is_err());
    }

    #[test]
    fn zero_build_id_is_rejected() {
        let mut header = [0u8; HEADER_SIZE];
        serialize_header(
            &mut header,
            4096,
            0,
            [0u8; BUILD_ID_LEN],
            b"scenario",
            0,
            Degradation::none(),
        );
        assert!(parse_header(&header).is_err());
    }

    #[test]
    fn staging_buffer_flushes_without_loss() {
        let mut staging = TestStaging::new(8);
        staging.write(b"abc");
        staging.write(b"defghijk");
        staging.flush();
        assert_eq!(staging.persisted, b"abcdefghijk");
    }

    #[test]
    fn staging_write_failure_degrades_without_panic() {
        let mut staging = TestStaging::new(4);
        staging.fail_next = true;
        staging.write(b"abcd");
        assert!(staging.degraded);
        staging.write(b"efgh");
        assert_eq!(staging.persisted, b"efgh");
    }

    struct TestStaging {
        buf: Vec<u8>,
        cap: usize,
        persisted: Vec<u8>,
        degraded: bool,
        fail_next: bool,
    }

    impl TestStaging {
        fn new(cap: usize) -> Self {
            Self {
                buf: Vec::new(),
                cap,
                persisted: Vec::new(),
                degraded: false,
                fail_next: false,
            }
        }

        fn write(&mut self, mut bytes: &[u8]) {
            while !bytes.is_empty() {
                let take = (self.cap - self.buf.len()).min(bytes.len());
                self.buf.extend_from_slice(&bytes[..take]);
                bytes = &bytes[take..];
                if self.buf.len() == self.cap {
                    self.flush();
                }
            }
        }

        fn flush(&mut self) {
            if self.buf.is_empty() {
                return;
            }
            if self.fail_next {
                self.fail_next = false;
                self.degraded = true;
                self.buf.clear();
                return;
            }
            self.persisted.extend_from_slice(&self.buf);
            self.buf.clear();
        }
    }
}
