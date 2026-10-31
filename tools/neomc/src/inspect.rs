// SPDX-License-Identifier: MPL-2.0
//! B1c (F4): `neomc inspect` — each tensor's dtype, dims, byte length, pages
//! and the buddy order its storage needs, plus the largest tensor. The kernel
//! sizes `MAX_ORDER` from that last line, so it is computed here the same way
//! the kernel allocates a tensor (`order_for_len`: pages rounded up to a
//! power of two), from the validated file alone.

use crate::format::{NeomResult, TENSOR_ENTRY_SIZE, read_u32_le, read_u64_le};
use crate::validator;

const PAGE: u64 = 4096;

/// One tensor's storage needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TensorRow {
    pub name: String,
    pub dtype: u32,
    pub dims: Vec<u64>,
    pub bytes: u64,
    pub pages: u64,
    pub order: u32,
}

/// Pages rounded up to a power of two: the buddy order a block of `bytes`
/// needs (the kernel's `tensor_logic::order_for_len`).
pub fn order_for_bytes(bytes: u64) -> u32 {
    let pages = bytes.div_ceil(PAGE).max(1);
    pages.next_power_of_two().trailing_zeros()
}

/// Validate `data` and list its tensors in table order.
pub fn inspect(data: &[u8]) -> NeomResult<Vec<TensorRow>> {
    validator::validate(data)?;
    let tensor_count = read_u32_le(data, 0x10) as usize;
    let table = read_u64_le(data, 0x20) as usize;
    let strings = read_u64_le(data, 0x38) as usize;
    let mut rows = Vec::with_capacity(tensor_count);
    for i in 0..tensor_count {
        let base = table + i * TENSOR_ENTRY_SIZE as usize;
        let rank = read_u32_le(data, base + 4) as usize;
        let dims = (0..rank)
            .map(|d| read_u64_le(data, base + 8 + 8 * d))
            .collect();
        let bytes = read_u64_le(data, base + 0x50);
        let name_off = read_u32_le(data, base + 0x58) as usize;
        let name_len = read_u32_le(data, base + 0x5C) as usize;
        let name =
            String::from_utf8_lossy(&data[strings + name_off..strings + name_off + name_len])
                .into_owned();
        rows.push(TensorRow {
            name,
            dtype: read_u32_le(data, base),
            dims,
            bytes,
            pages: bytes.div_ceil(PAGE),
            order: order_for_bytes(bytes),
        });
    }
    Ok(rows)
}

/// The report `neomc inspect` prints; the last line names the minimum
/// `MAX_ORDER` that holds every tensor in one buddy block.
pub fn render(rows: &[TensorRow]) -> String {
    let mut out = String::from("name dtype dims bytes pages order\n");
    for row in rows {
        let dims: Vec<String> = row.dims.iter().map(u64::to_string).collect();
        out.push_str(&format!(
            "{} {} [{}] {} {} {}\n",
            row.name,
            row.dtype,
            dims.join(","),
            row.bytes,
            row.pages,
            row.order
        ));
    }
    let total: u64 = rows.iter().map(|r| r.bytes).sum();
    // What the tensors occupy once each is one power-of-two buddy block.
    let buddy: u64 = rows.iter().map(|r| PAGE << r.order).sum();
    if let Some(largest) = rows.iter().max_by_key(|r| r.bytes) {
        out.push_str(&format!(
            "tensors={} weight_bytes={} buddy_bytes={} largest={} largest_bytes={} min_max_order={}\n",
            rows.len(),
            total,
            buddy,
            largest.name,
            largest.bytes,
            largest.order
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_rounds_pages_up_to_a_power_of_two() {
        assert_eq!(order_for_bytes(1), 0);
        assert_eq!(order_for_bytes(4096), 0);
        assert_eq!(order_for_bytes(4097), 1);
        // The pinned 0.5B's largest tensor, output.weight (Q8_0, 151,936 x 896):
        // 35,314 pages round to 65,536 = order 16; order 15 (32,768 pages,
        // 128 MiB) is too small.
        assert_eq!(144_643_072u64.div_ceil(4096), 35_314);
        assert_eq!(order_for_bytes(144_643_072), 16);
        assert_eq!(order_for_bytes(128 * 1024 * 1024), 15);
        assert_eq!(order_for_bytes(128 * 1024 * 1024 + 1), 16);
    }

    #[test]
    fn inspects_the_kv_persist_kat_fixture() {
        let data = crate::fixtures::kv_persist_kat_inventory_bytes().expect("fixture");
        let rows = inspect(&data).expect("inspect");
        assert_eq!(rows.len(), 12);
        let q = rows
            .iter()
            .find(|r| r.name == "blk.0.attn_q.weight")
            .unwrap();
        assert_eq!(
            (q.dims.clone(), q.bytes, q.pages, q.order),
            (vec![128, 128], 65_536, 16, 4)
        );
        let report = render(&rows);
        assert!(report.ends_with("min_max_order=4\n"), "{report}");
        // 4 x 64 KiB + 3 x 32 KiB + 2 x 8 KiB (two pages, order 1) + 3 x
        // 4 KiB (one page each), each one block.
        assert!(report.contains("buddy_bytes=389120 "), "{report}");
    }
}
