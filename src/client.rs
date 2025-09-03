//! A synchronous VICI client over a UNIX domain socket.
//!
//! This client keeps things simple and blocking by default. It still exposes
//! the underlying file descriptor so advanced users can integrate it into
//! their own event loop if desired.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
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
    /// Connect to a VICI UNIX socket.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the UNIX socket (typically `/var/run/charon.vici`)
    ///
    /// # Returns
    ///
    /// Returns a connected `Client` on success, or an `Error` if the connection fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::Client;
    ///
    /// let client = Client::connect("/var/run/charon.vici")?;
    /// ```
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        Ok(Self { stream })
    }

    /// Returns the raw file descriptor for integration with `select`/`poll`.
    ///
    /// This is useful for integrating the client into custom event loops
    /// or for monitoring multiple file descriptors simultaneously.
    ///
    /// # Platform support
    ///
    /// Linux/Unix only.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::Client;
    ///
    /// let client = Client::connect("/var/run/charon.vici")?;
    /// let fd = client.as_raw_fd();
    /// // Use fd with select(), poll(), or epoll
    /// ```
    pub fn as_raw_fd(&self) -> i32 {
        self.stream.as_raw_fd()
    }

    /// Set the read timeout for socket operations.
    ///
    /// When set, read operations will fail with a timeout error if they
    /// don't complete within the specified duration.
    ///
    /// # Arguments
    ///
    /// * `to` - The timeout duration, or `None` to disable timeouts (blocking mode)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if setting the timeout fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use rustici::Client;
    ///
    /// let client = Client::connect("/var/run/charon.vici")?;
    /// client.set_read_timeout(Some(Duration::from_secs(5)))?;
    /// ```
    pub fn set_read_timeout(&self, to: Option<Duration>) -> Result<()> {
        self.stream.set_read_timeout(to).map_err(Error::Io)
    }

    /// Set the write timeout for socket operations.
    ///
    /// When set, write operations will fail with a timeout error if they
    /// don't complete within the specified duration.
    ///
    /// # Arguments
    ///
    /// * `to` - The timeout duration, or `None` to disable timeouts (blocking mode)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if setting the timeout fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use rustici::Client;
    ///
    /// let client = Client::connect("/var/run/charon.vici")?;
    /// client.set_write_timeout(Some(Duration::from_secs(5)))?;
    /// ```
    pub fn set_write_timeout(&self, to: Option<Duration>) -> Result<()> {
        self.stream.set_write_timeout(to).map_err(Error::Io)
    }

    /// Send a simple RPC-style command and await its response.
    ///
    /// This method sends a command request and waits for the corresponding
    /// response. Any unsolicited events received while waiting are silently
    /// ignored.
    ///
    /// # Arguments
    ///
    /// * `command` - The VICI command name (e.g., "version", "list-sas", "initiate")
    /// * `request` - The message payload for the command
    ///
    /// # Returns
    ///
    /// Returns the response `Message` on success. If the command is unknown
    /// to the daemon, returns `Error::UnknownCommand`. Empty responses are
    /// returned as empty `Message` objects.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::{Client, wire::Message};
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// let response = client.call("version", &Message::new())?;
    /// println!("Response: {}", response);
    /// ```
    pub fn call(&mut self, command: &str, request: &Message) -> Result<Message> {
        let pkt = Packet {
            ty: PacketType::CmdRequest,
            name: Some(command.to_string()),
            message: Some(request.clone()),
        };
        self.send_packet(&pkt)?;

        loop {
            let pkt = self.recv_packet()?;
            match pkt.ty {
                PacketType::CmdResponse => {
                    // Some VICI commands legitimately return an empty response body.
                    // Treat a missing message as an empty Message instead of a protocol error.
                    return Ok(pkt.message.unwrap_or_default());
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

    /// Register to receive events of a specific type.
    ///
    /// After successful registration, the client will receive events of the
    /// specified type which can be retrieved using `next_event()` or related methods.
    ///
    /// # Arguments
    ///
    /// * `name` - The event type name (e.g., "ike-updown", "child-updown", "log")
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if registration succeeds, or an error if the event
    /// type is unknown or registration fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::Client;
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// client.register_event("ike-updown")?;
    /// // Now the client will receive IKE SA up/down events
    /// ```
    pub fn register_event(&mut self, name: &str) -> Result<()> {
        let pkt = Packet {
            ty: PacketType::EventRegister,
            name: Some(name.to_string()),
            message: None,
        };
        self.send_packet(&pkt)?;
        let resp = self.recv_packet()?;
        match resp.ty {
            PacketType::EventConfirm => Ok(()),
            PacketType::EventUnknown => Err(Error::Protocol("event registration failed")),
            _ => Err(Error::Protocol("unexpected packet after event register")),
        }
    }

    /// Unregister from receiving events of a specific type.
    ///
    /// After successful unregistration, the client will no longer receive
    /// events of the specified type.
    ///
    /// # Arguments
    ///
    /// * `name` - The event type name to unregister from
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if unregistration succeeds, or an error if it fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::Client;
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// client.register_event("log")?;
    /// // ... receive some log events ...
    /// client.unregister_event("log")?;
    /// // No more log events will be received
    /// ```
    pub fn unregister_event(&mut self, name: &str) -> Result<()> {
        let pkt = Packet {
            ty: PacketType::EventUnregister,
            name: Some(name.to_string()),
            message: None,
        };
        self.send_packet(&pkt)?;
        let resp = self.recv_packet()?;
        match resp.ty {
            PacketType::EventConfirm => Ok(()),
            PacketType::EventUnknown => Err(Error::Protocol("event deregistration failed")),
            _ => Err(Error::Protocol("unexpected packet after event unregister")),
        }
    }

    /// Execute a streaming command that yields multiple events before completing.
    ///
    /// Some VICI commands (like "list-sas", "list-conns") stream multiple event
    /// packets before sending a final response. This method handles the streaming
    /// protocol and invokes the callback for each event.
    ///
    /// # Arguments
    ///
    /// * `command` - The streaming command name
    /// * `request` - The message payload for the command
    /// * `on_event` - Callback invoked for each streamed event with (event_name, event_message)
    ///
    /// # Returns
    ///
    /// Returns the final response message after all events have been streamed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::{Client, wire::Message};
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// let response = client.call_streaming("list-sas", &Message::new(), |name, msg| {
    ///     println!("Event {}: {}", name, msg);
    /// })?;
    /// println!("Final response: {}", response);
    /// ```
    pub fn call_streaming<F>(
        &mut self,
        command: &str,
        request: &Message,
        mut on_event: F,
    ) -> Result<Message>
    where
        F: FnMut(&str, &Message),
    {
        let pkt = Packet {
            ty: PacketType::CmdRequest,
            name: Some(command.to_string()),
            message: Some(request.clone()),
        };
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
                    // Final response may be empty; surface it as an empty Message.
                    return Ok(pkt.message.unwrap_or_default());
                }
                PacketType::CmdUnknown => return Err(Error::UnknownCommand(command.to_string())),
                _ => {
                    return Err(Error::Protocol(
                        "unexpected packet while awaiting streamed response",
                    ))
                }
            }
        }
    }

    /// Block until the next event message arrives.
    ///
    /// This method blocks indefinitely waiting for an event. You must have
    /// registered for at least one event type using `register_event()` to
    /// receive events.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (event_name, event_message) when an event arrives.
    ///
    /// # Note
    ///
    /// This method will block forever if no events arrive. Consider using
    /// `next_event_with_timeout()` or `try_next_event()` for non-blocking
    /// alternatives.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustici::Client;
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// client.register_event("ike-updown")?;
    ///
    /// loop {
    ///     let (event_name, message) = client.next_event()?;
    ///     println!("Received event: {}", event_name);
    /// }
    /// ```
    pub fn next_event(&mut self) -> Result<(String, Message)> {
        loop {
            let pkt = self.recv_packet()?;
            if let PacketType::Event = pkt.ty {
                let name = pkt.name.ok_or(Error::Protocol("event without name"))?;
                let msg = pkt
                    .message
                    .ok_or(Error::Protocol("event without message"))?;
                return Ok((name, msg));
            }
        }
    }

    /// Block until the next event message arrives or timeout occurs.
    ///
    /// This method respects the read timeout set via `set_read_timeout()`.
    /// If no timeout is set, it behaves identically to `next_event()`.
    ///
    /// # Returns
    ///
    /// Returns `Ok((event_name, event_message))` when an event arrives, or
    /// `Err(Error::Timeout)` if the timeout expires before an event is received.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use rustici::{Client, error::Error};
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// client.set_read_timeout(Some(Duration::from_secs(1)))?;
    /// client.register_event("ike-updown")?;
    ///
    /// loop {
    ///     match client.next_event_with_timeout() {
    ///         Ok((name, msg)) => println!("Got event: {}", name),
    ///         Err(Error::Timeout) => {
    ///             println!("No event within timeout period");
    ///             // Check shutdown flags or do other work
    ///         }
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub fn next_event_with_timeout(&mut self) -> Result<(String, Message)> {
        loop {
            let pkt = match self.recv_packet() {
                Ok(pkt) => pkt,
                Err(e) => {
                    // Convert I/O timeout errors to our Timeout error
                    if let Error::Io(ref io_err) = e {
                        if io_err.kind() == std::io::ErrorKind::TimedOut
                            || io_err.kind() == std::io::ErrorKind::WouldBlock
                        {
                            return Err(Error::Timeout);
                        }
                    }
                    return Err(e);
                }
            };

            if let PacketType::Event = pkt.ty {
                let name = pkt.name.ok_or(Error::Protocol("event without name"))?;
                let msg = pkt
                    .message
                    .ok_or(Error::Protocol("event without message"))?;
                return Ok((name, msg));
            }
        }
    }

    /// Try to receive the next event with a specific timeout.
    ///
    /// This is a convenience method that temporarily sets the read timeout,
    /// attempts to receive an event, and then restores the previous timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum duration to wait for an event
    ///
    /// # Returns
    ///
    /// Returns `Ok((event_name, event_message))` if an event arrives within
    /// the timeout, or `Err(Error::Timeout)` if the timeout expires.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use rustici::{Client, error::Error};
    ///
    /// let mut client = Client::connect("/var/run/charon.vici")?;
    /// client.register_event("log")?;
    ///
    /// match client.try_next_event(Duration::from_millis(500)) {
    ///     Ok((name, msg)) => println!("Got event: {}", name),
    ///     Err(Error::Timeout) => println!("No event within 500ms"),
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn try_next_event(&mut self, timeout: Duration) -> Result<(String, Message)> {
        // Save current timeout
        let previous_timeout = self.stream.read_timeout().ok().flatten();

        // Set new timeout
        self.set_read_timeout(Some(timeout))?;

        // Try to get next event
        let result = self.next_event_with_timeout();

        // Restore previous timeout
        self.set_read_timeout(previous_timeout)?;

        result
    }

    /// Send a packet (encodes transport frame).
    fn send_packet(&mut self, pkt: &Packet) -> Result<()> {
        let mut data = Vec::new();
        data.push(pkt.ty as u8);
        if pkt.ty.is_named() {
            let name = pkt
                .name
                .as_ref()
                .ok_or(Error::Protocol("named packet missing name"))?;
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
    if input.is_empty() {
        return Err(Error::Protocol("unexpected EOF reading u8"));
    }
    Ok((input[0], &input[1..]))
}

fn decode_name(input: &[u8]) -> Result<(String, &[u8])> {
    if input.is_empty() {
        return Err(Error::Protocol("unexpected EOF reading name length"));
    }
    let len = input[0] as usize;
    let input = &input[1..];
    if input.len() < len {
        return Err(Error::Protocol("unexpected EOF reading name bytes"));
    }
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
