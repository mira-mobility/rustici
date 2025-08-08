//! A synchronous VICI client over a UNIX domain socket.
//!
//! This client keeps things simple and blocking by default. It still exposes
//! the underlying file descriptor so advanced users can integrate it into
//! their own event loop if desired.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::packet::{Packet, PacketType};
use crate::wire::Message;

/// The default charon VICI socket path.
pub const DEFAULT_SOCKET: &str = "/var/run/charon.vici";

/// A simple synchronous client.
pub struct Client {
    stream: UnixStream,
}

impl Client {
    /// Connect to a VICI UNIX socket (e.g., `/var/run/charon.vici`).
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        Ok(Self { stream })
    }

    /// Returns the raw file descriptor (for integration with `select`/`poll`). Linux/Unix only.
    pub fn as_raw_fd(&self) -> i32 { self.stream.as_raw_fd() }

    /// Set read/write timeouts.
    pub fn set_read_timeout(&self, to: Option<Duration>) -> Result<()> {
        self.stream.set_read_timeout(to).map_err(Error::Io)
    }
    /// Set write timeout on the underlying socket.
    pub fn set_write_timeout(&self, to: Option<Duration>) -> Result<()> {
        self.stream.set_write_timeout(to).map_err(Error::Io)
    }

    /// Send a *simple* RPC-style command and await its response. Any unsolicited events
    /// received while waiting are ignored.
    pub fn call(&mut self, command: &str, request: &Message) -> Result<Message> {
        let pkt = Packet { ty: PacketType::CmdRequest, name: Some(command.to_string()), message: Some(request.clone()) };
        self.send_packet(&pkt)?;

        loop {
            let pkt = self.recv_packet()?;
            match pkt.ty {
                PacketType::CmdResponse => {
                    return pkt.message.ok_or(Error::Protocol("response without message"));
                }
                PacketType::CmdUnknown => {
                    return Err(Error::UnknownCommand(command.to_string()));
                }
                PacketType::Event => {
                    // Ignore unsolicited events here.
                    continue;
                }
                _ => return Err(Error::Protocol("unexpected packet while awaiting response")),
            }
        }
    }

    /// Register for an event name, returning Ok(()) if the daemon confirms.
    pub fn register_event(&mut self, name: &str) -> Result<()> {
        let pkt = Packet { ty: PacketType::EventRegister, name: Some(name.to_string()), message: None };
        self.send_packet(&pkt)?;
        let resp = self.recv_packet()?;
        match resp.ty {
            PacketType::EventConfirm => Ok(()),
            PacketType::EventUnknown => Err(Error::Protocol("event registration failed")),
            _ => Err(Error::Protocol("unexpected packet after event register")),
        }
    }

    /// Unregister from an event name.
    pub fn unregister_event(&mut self, name: &str) -> Result<()> {
        let pkt = Packet { ty: PacketType::EventUnregister, name: Some(name.to_string()), message: None };
        self.send_packet(&pkt)?;
        let resp = self.recv_packet()?;
        match resp.ty {
            PacketType::EventConfirm => Ok(()),
            PacketType::EventUnknown => Err(Error::Protocol("event deregistration failed")),
            _ => Err(Error::Protocol("unexpected packet after event unregister")),
        }
    }


    /// Execute a *streaming* command that yields one or more EVENT packets and
    /// finally returns a CMD_RESPONSE. For each EVENT, the provided callback is
    /// invoked with `(event_name, event_message)`.
    pub fn call_streaming<F>(&mut self, command: &str, request: &Message, mut on_event: F) -> Result<Message>
    where
        F: FnMut(&str, &Message),
    {
        let pkt = Packet { ty: PacketType::CmdRequest, name: Some(command.to_string()), message: Some(request.clone()) };
        self.send_packet(&pkt)?;

        loop {
            let pkt = self.recv_packet()?;
            match pkt.ty {
                PacketType::Event => {
                    if let (Some(name), Some(msg)) = (pkt.name.as_ref(), pkt.message.as_ref()) {
                        on_event(name, msg);
                    } else {
                        return Err(Error::Protocol("event without name or message"));
                    }
                }
                PacketType::CmdResponse => {
                    return pkt.message.ok_or(Error::Protocol("response without message"));
                }
                PacketType::CmdUnknown => return Err(Error::UnknownCommand(command.to_string())),
                _ => return Err(Error::Protocol("unexpected packet while awaiting streamed response")),
            }
        }
    }
    /// Block until the next event message arrives. Returns the (event name, message).
    pub fn next_event(&mut self) -> Result<(String, Message)> {
        loop {
            let pkt = self.recv_packet()?;
            if let PacketType::Event = pkt.ty {
                let name = pkt.name.ok_or(Error::Protocol("event without name"))?;
                let msg = pkt.message.ok_or(Error::Protocol("event without message"))?;
                return Ok((name, msg));
            }
        }
    }

    /// Send a packet (encodes transport frame).
    fn send_packet(&mut self, pkt: &Packet) -> Result<()> {
        let mut data = Vec::new();
        data.push(pkt.ty as u8);
        if pkt.ty.is_named() {
            let name = pkt.name.as_ref().ok_or(Error::Protocol("named packet missing name"))?;
            encode_name(&mut data, name)?;
        }
        if let Some(msg) = &pkt.message {
            let bytes = msg.encode()?;
            data.extend_from_slice(&bytes);
        }
        let len = data.len();
        if len > (512 * 1024) {
            return Err(Error::TooLong("packet"));
        }
        let mut frame = Vec::with_capacity(4 + len);
        frame.extend_from_slice(&(len as u32).to_be_bytes());
        frame.extend_from_slice(&data);
        self.stream.write_all(&frame)?;
        Ok(())
    }

    /// Receive the *next* packet from the stream (decodes one transport frame).
    fn recv_packet(&mut self) -> Result<Packet> {
        // Read 4-byte length header (big endian)
        let mut len_hdr = [0u8; 4];
        self.stream.read_exact(&mut len_hdr)?;
        let len = u32::from_be_bytes(len_hdr) as usize;
        if len > (512 * 1024) {
            return Err(Error::TooLong("frame"));
        }
        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf)?;
        // Parse packet
        let (ty_u8, mut rest) = decode_u8(&buf)?;
        let ty = match ty_u8 {
            0 => PacketType::CmdRequest,
            1 => PacketType::CmdResponse,
            2 => PacketType::CmdUnknown,
            3 => PacketType::EventRegister,
            4 => PacketType::EventUnregister,
            5 => PacketType::EventConfirm,
            6 => PacketType::EventUnknown,
            7 => PacketType::Event,
            _ => return Err(Error::Protocol("unknown packet type")),
        };
        let name = if ty.is_named() {
            let (nm, r) = decode_name(rest)?;
            rest = r;
            Some(nm)
        } else {
            None
        };
        let message = if !rest.is_empty() {
            Some(Message::decode(rest)?)
        } else {
            None
        };
        Ok(Packet { ty, name, message })
    }
}

// --- Small local helpers (mirror what's in wire.rs but private here) ---

fn decode_u8(input: &[u8]) -> Result<(u8, &[u8])> {
    if input.is_empty() { return Err(Error::Protocol("unexpected EOF reading u8")); }
    Ok((input[0], &input[1..]))
}

fn decode_name(input: &[u8]) -> Result<(String, &[u8])> {
    if input.is_empty() { return Err(Error::Protocol("unexpected EOF reading name length")); }
    let len = input[0] as usize;
    let input = &input[1..];
    if input.len() < len { return Err(Error::Protocol("unexpected EOF reading name bytes")); }
    let name = String::from_utf8(input[..len].to_vec())?;
    Ok((name, &input[len..]))
}

fn encode_name(out: &mut Vec<u8>, name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.len() > u8::MAX as usize {
        return Err(Error::TooLong("packet name"));
    }
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    Ok(())
}
