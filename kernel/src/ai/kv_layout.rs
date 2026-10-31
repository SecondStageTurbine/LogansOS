// SPDX-License-Identifier: MPL-2.0
//! Shared Qwen2 KV-cache storage layout helpers.

use super::forward_logic::ForwardLayerError;

pub fn storage_index(
    head: usize,
    pos: usize,
    dim: usize,
    max_context: usize,
    head_dim: usize,
) -> Result<usize, ForwardLayerError> {
    checked_add(
        checked_mul(checked_add(checked_mul(head, max_context)?, pos)?, head_dim)?,
        dim,
    )
}

pub fn token_index(head: usize, dim: usize, head_dim: usize) -> Result<usize, ForwardLayerError> {
    checked_add(checked_mul(head, head_dim)?, dim)
}

pub fn visit_token<E, F>(heads: usize, head_dim: usize, mut visit: F) -> Result<(), E>
where
    E: From<ForwardLayerError>,
    F: FnMut(usize, usize, usize) -> Result<(), E>,
{
    let mut head = 0usize;
    while head < heads {
        let mut dim = 0usize;
        while dim < head_dim {
            let token_idx = token_index(head, dim, head_dim)?;
            visit(head, dim, token_idx)?;
            dim += 1;
        }
        head += 1;
    }
    Ok(())
}

pub fn visit_active_prefix<E, F>(
    heads: usize,
    used_len: usize,
    max_context: usize,
    head_dim: usize,
    mut visit: F,
) -> Result<(), E>
where
    E: From<ForwardLayerError>,
    F: FnMut(usize, usize, usize, usize) -> Result<(), E>,
{
    let mut head = 0usize;
    while head < heads {
        let mut pos = 0usize;
        while pos < used_len {
            let mut dim = 0usize;
            while dim < head_dim {
                let storage_idx = storage_index(head, pos, dim, max_context, head_dim)?;
                visit(head, pos, dim, storage_idx)?;
                dim += 1;
            }
            pos += 1;
        }
        head += 1;
    }
    Ok(())
}

fn checked_add(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_add(rhs).ok_or(ForwardLayerError::Shape)
}

fn checked_mul(lhs: usize, rhs: usize) -> Result<usize, ForwardLayerError> {
    lhs.checked_mul(rhs).ok_or(ForwardLayerError::Shape)
}
