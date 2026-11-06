// SPDX-License-Identifier: MIT
//! Stand-in for `hyper-util` 0.1.21 on a content process: see this crate's
//! Cargo.toml.

/// The client module, as far as `servo-net-traits` names it.
pub mod client {
    /// The legacy client, as far as `servo-net-traits` names it.
    pub mod legacy {
        /// An HTTP client error. Never constructed: there is no HTTP client.
        #[derive(Debug)]
        pub struct Error(());

        impl std::fmt::Display for Error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("no HTTP client in this process")
            }
        }

        impl std::error::Error for Error {}
    }
}
