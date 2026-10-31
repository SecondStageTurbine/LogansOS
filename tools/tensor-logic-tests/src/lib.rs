// SPDX-License-Identifier: MPL-2.0
//! Host tests for Phase 5 tensor descriptor logic.
#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/ai/tensor_logic.rs"]
mod tensor_logic;

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use super::tensor_logic::{Dtype, Shape, TensorError, order_for_len, validate_data_len};

    #[test]
    fn dtype_sizes_match_build1_abi() {
        assert_eq!(Dtype::F32.size_bytes(), 4);
        assert_eq!(Dtype::F16.size_bytes(), 2);
        assert_eq!(Dtype::BF16.size_bytes(), 2);
        assert_eq!(Dtype::I32.size_bytes(), 4);
        assert_eq!(Dtype::I8.size_bytes(), 1);
        assert_eq!(Dtype::U8.size_bytes(), 1);
        assert_eq!(Dtype::Bool.size_bytes(), 1);
        assert_eq!(Dtype::from_abi(Dtype::BOOL_ABI), Ok(Dtype::Bool));
        assert_eq!(Dtype::from_abi(99), Err(TensorError::InvalidDtype));
    }

    #[test]
    fn shape_validation_accepts_scalar_and_rejects_bad_rank_or_zero_dim() {
        let scalar = Shape::new(0, [0; 8]);
        assert!(matches!(scalar, Ok(shape) if shape.byte_len(Dtype::F32) == Ok(4)));

        assert_eq!(Shape::new(9, [1; 8]), Err(TensorError::InvalidShape));

        let mut dims = [1u64; 8];
        dims[2] = 0;
        assert_eq!(Shape::new(3, dims), Err(TensorError::InvalidShape));
    }

    #[test]
    fn byte_len_must_match_shape_and_dtype() {
        let mut dims = [0u64; 8];
        dims[0] = 2;
        dims[1] = 3;
        let shape = Shape::new(2, dims);
        assert!(shape.is_ok());
        let Ok(shape) = shape else {
            return;
        };
        assert_eq!(validate_data_len(shape, Dtype::F32, 24), Ok(24));
        assert_eq!(
            validate_data_len(shape, Dtype::F32, 23),
            Err(TensorError::InvalidLength)
        );
    }

    #[test]
    fn buddy_order_rounds_up_to_power_of_two_pages() {
        assert_eq!(order_for_len(1), Ok(0));
        assert_eq!(order_for_len(4096), Ok(0));
        assert_eq!(order_for_len(4097), Ok(1));
        assert_eq!(order_for_len(4096 * 3), Ok(2));
        assert_eq!(order_for_len(0), Err(TensorError::InvalidLength));
    }

    #[test]
    fn arc_refcount_arithmetic_matches_share_drop_model() {
        let shape = Shape::new(0, [0; 8]);
        assert!(shape.is_ok());
        let Ok(shape) = shape else {
            return;
        };
        let shape = Arc::new(shape);
        assert_eq!(Arc::strong_count(&shape), 1);
        let shared = shape.clone();
        assert_eq!(Arc::strong_count(&shape), 2);
        drop(shared);
        assert_eq!(Arc::strong_count(&shape), 1);
    }
}
