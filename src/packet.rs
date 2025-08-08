//! Packet layer definitions (outside the message codec).

use crate::wire::Message;
use std::fmt;

/// Top-level packet types in the VICI protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// A named request message (client -> server).
    CmdRequest = 0,
    /// An unnamed response message (server -> client).
    CmdResponse = 1,
    /// An unnamed response indicating the command was unknown.
    CmdUnknown = 2,
    /// A named event registration request (client -> server).
    EventRegister = 3,
    /// A named event deregistration request (client -> server).
    EventUnregister = 4,
    /// An unnamed response for successful event (de-)registration.
    EventConfirm = 5,
    /// An unnamed response if event (de-)registration failed.
    EventUnknown = 6,
    /// A named event message (server -> client).
    Event = 7,
}

impl PacketType {
    /// Whether this packet carries a "name" field (single-byte length + ASCII bytes).
    pub fn is_named(self) -> bool {
        matches!(self,
            PacketType::CmdRequest |
            PacketType::EventRegister |
            PacketType::EventUnregister |
            PacketType::Event
        )
    }
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A parsed packet ready to be consumed by the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// The packet type.
    pub ty: PacketType,
    /// Optional name (command name or event name), present if `ty.is_named()`.
    pub name: Option<String>,
    /// Optional hierarchical message.
    pub message: Option<Message>,
}

impl Packet {
    /// Create a new packet with optional name/message.
    pub fn new(ty: PacketType, name: Option<String>, message: Option<Message>) -> Self {
        Self { ty, name, message }
    }
}
