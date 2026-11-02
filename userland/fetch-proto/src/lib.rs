// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 5: the fetch broker's protocol, with no I/O.
//!
//! `docs/THINK-2026-10-30-ot61-r5-fetch-broker.md` is the spec:
//! - [`wire`]: the messages a content process and the broker exchange over
//!   their channel (§2), with [`wire::encode`] / [`wire::decode`];
//! - [`policy`]: what the broker refuses before and after it dials (§3);
//! - [`http`]: an HTTP/1.1 response head and its body framing (§4);
//! - [`chunked`]: an HTTP/1.1 `Transfer-Encoding: chunked` decoder (§4).
//!
//! The broker (`userland/fetch-broker`) and the browser's content side both
//! use this crate, so the two ends cannot drift apart; the byte oracles in the
//! tests are written out by hand, independent of the encoder and decoder.

#![no_std]

extern crate alloc;

pub mod chunked;
pub mod http;
pub mod policy;
pub mod wire;

/// The kernel's NetDial rules, the same file `net-dial-public-only` enforces
/// in the dial path, so [`policy::ip_allowed`] and the kernel can't disagree.
#[allow(dead_code)]
#[path = "../../../kernel/src/sched/net_dial_cap_logic.rs"]
mod dial_logic;
