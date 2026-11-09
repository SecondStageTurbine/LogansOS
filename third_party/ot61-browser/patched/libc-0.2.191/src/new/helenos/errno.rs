// SPDX-License-Identifier: MIT OR Apache-2.0
//! HelenOS errno handling
//!
//! * Header file: <https://github.com/HelenOS/helenos/tree/master/uspace/lib/c/include/errno.h>

extern "C" {
    pub fn __errno() -> *mut crate::errno_t;
}
