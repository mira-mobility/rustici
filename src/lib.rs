//! rustici — a minimal Rust client for the strongSwan VICI protocol.
//!
//! This crate implements the **wire format** and a **synchronous client** for VICI
//! over a UNIX domain socket (default: `/var/run/charon.vici`).
//!
//! ### Status
//! This is an early, intentionally small implementation. It focuses on correctness
//! of the wire codec and a straightforward blocking client. It does **not** depend
//! on libstrongswan or davici. No external crates are used.
//!
//! See the `examples/` folder for usage.
//!
//! ### Licensing note
//! This crate implements an open protocol (VICI). It does not copy code from
//! strongSwan. You may use it under MIT or Apache-2.0.
//!
//! ### References
//! - strongSwan VICI plugin docs (protocol overview).
//! - The VICI README describes packet/message formats.
//!
//! **Not an official project of the strongSwan team.**
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod client;
pub mod error;
pub mod packet;
pub mod wire;

// Re-export primary types
pub use crate::client::Client;
pub use crate::packet::{Packet, PacketType};
pub use crate::wire::Message;
