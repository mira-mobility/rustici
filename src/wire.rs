//! Message codec (hierarchical sections, lists, key/value pairs).
//!
//! Encoding summary (as specified by the VICI protocol):
//! - Element types are 8-bit values:
//!   - 1: SECTION_START (has a name)
//!   - 2: SECTION_END
//!   - 3: KEY_VALUE (has name + value)
//!   - 4: LIST_START (has a name)
//!   - 5: LIST_ITEM (has a value)
//!   - 6: LIST_END
//! - Names are ASCII strings preceded by an 8-bit length (not NUL-terminated).
//! - Values are raw blobs preceded by a 16-bit big-endian length.
//!
//! Transport framing (outside of this module) wraps the packet with a 32-bit
//! big-endian length field, followed by: packet type (u8), optional name, and
//! the encoded message bytes.

use crate::error::{Error, Result};
use std::fmt;

/// A single message element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    /// Begin a named section.
    SectionStart(String),
    /// End the most recently opened section.
    SectionEnd,
    /// A key/value pair.
    KeyValue(String, Vec<u8>),
    /// Begin a named list.
    ListStart(String),
    /// A list item value.
    ListItem(Vec<u8>),
    /// End the most recently opened list.
    ListEnd,
}

impl Element {
    fn encode_into(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Element::SectionStart(name) => {
                out.push(1);
                encode_name(out, name)?;
            }
            Element::SectionEnd => out.push(2),
            Element::KeyValue(name, val) => {
                out.push(3);
                encode_name(out, name)?;
                encode_value(out, val)?;
            }
            Element::ListStart(name) => {
                out.push(4);
                encode_name(out, name)?;
            }
            Element::ListItem(val) => {
                out.push(5);
                encode_value(out, val)?;
            }
            Element::ListEnd => out.push(6),
        }
        Ok(())
    }
}

/// A full message consisting of a flat sequence of elements.
/// The sequence must be *balanced* with regards to sections and lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    elements: Vec<Element>,
}

impl Message {
    /// Create an empty message.
    pub fn new() -> Self { Self { elements: Vec::new() } }

    /// Borrow inner elements.
    pub fn elements(&self) -> &[Element] { &self.elements }

    /// Push a raw element.
    pub fn push(&mut self, el: Element) { self.elements.push(el); }

    /// Convenience: add key/value where the value is a string.
    pub fn kv_str(mut self, name: impl Into<String>, value: impl AsRef<str>) -> Self {
        self.elements.push(Element::KeyValue(name.into(), value.as_ref().as_bytes().to_vec()));
        self
    }

    /// Convenience: add key/value where the value is raw bytes.
    pub fn kv_bytes(mut self, name: impl Into<String>, value: impl AsRef<[u8]>) -> Self {
        self.elements.push(Element::KeyValue(name.into(), value.as_ref().to_vec()));
        self
    }

    /// Begin a section.
    pub fn section_start(mut self, name: impl Into<String>) -> Self {
        self.elements.push(Element::SectionStart(name.into()));
        self
    }

    /// End a section.
    pub fn section_end(mut self) -> Self {
        self.elements.push(Element::SectionEnd);
        self
    }

    /// Begin a list.
    pub fn list_start(mut self, name: impl Into<String>) -> Self {
        self.elements.push(Element::ListStart(name.into()));
        self
    }

    /// Add a list item (string value convenience).
    pub fn list_item_str(mut self, value: impl AsRef<str>) -> Self {
        self.elements.push(Element::ListItem(value.as_ref().as_bytes().to_vec()));
        self
    }

    /// Add a list item (raw bytes).
    pub fn list_item_bytes(mut self, value: impl AsRef<[u8]>) -> Self {
        self.elements.push(Element::ListItem(value.as_ref().to_vec()));
        self
    }

    /// End a list.
    pub fn list_end(mut self) -> Self {
        self.elements.push(Element::ListEnd);
        self
    }

    /// Encode this message into bytes.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.elements.len() * 8);
        for el in &self.elements {
            el.encode_into(&mut out)?;
        }
        Ok(out)
        }

    /// Decode a message from bytes.
    pub fn decode(mut bytes: &[u8]) -> Result<Self> {
        let mut elements = Vec::new();
        while !bytes.is_empty() {
            let (el, rest) = decode_element(bytes)?;
            elements.push(el);
            bytes = rest;
        }
        Ok(Self { elements })
    }
}

fn encode_name(out: &mut Vec<u8>, name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.len() > u8::MAX as usize {
        return Err(Error::TooLong("element name"));
    }
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    Ok(())
}

fn encode_value(out: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    if value.len() > u16::MAX as usize {
        return Err(Error::TooLong("element value"));
    }
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn decode_u8(input: &[u8]) -> Result<(u8, &[u8])> {
    if input.is_empty() { return Err(Error::Protocol("unexpected EOF reading u8")); }
    Ok((input[0], &input[1..]))
}

fn decode_be_u16(input: &[u8]) -> Result<(u16, &[u8])> {
    if input.len() < 2 { return Err(Error::Protocol("unexpected EOF reading u16")); }
    let v = u16::from_be_bytes([input[0], input[1]]);
    Ok((v, &input[2..]))
}

fn take(input: &[u8], n: usize) -> Result<(&[u8], &[u8])> {
    if input.len() < n { return Err(Error::Protocol("unexpected EOF taking slice")); }
    Ok((&input[..n], &input[n..]))
}

fn decode_name(input: &[u8]) -> Result<(String, &[u8])> {
    let (len, input) = decode_u8(input)?;
    let (name_bytes, rest) = take(input, len as usize)?;
    Ok((String::from_utf8(name_bytes.to_vec())?, rest))
}

fn decode_value(input: &[u8]) -> Result<(Vec<u8>, &[u8])> {
    let (len, input) = decode_be_u16(input)?;
    let (value, rest) = take(input, len as usize)?;
    Ok((value.to_vec(), rest))
}

fn decode_element(input: &[u8]) -> Result<(Element, &[u8])> {
    let (tag, input) = decode_u8(input)?;
    match tag {
        1 => { // SECTION_START
            let (name, rest) = decode_name(input)?;
            Ok((Element::SectionStart(name), rest))
        }
        2 => Ok((Element::SectionEnd, input)),
        3 => {
            let (name, input) = decode_name(input)?;
            let (value, rest) = decode_value(input)?;
            Ok((Element::KeyValue(name, value), rest))
        }
        4 => {
            let (name, rest) = decode_name(input)?;
            Ok((Element::ListStart(name), rest))
        }
        5 => {
            let (value, rest) = decode_value(input)?;
            Ok((Element::ListItem(value), rest))
        }
        6 => Ok((Element::ListEnd, input)),
        _ => Err(Error::Protocol("unknown message element tag")),
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for el in &self.elements {
            match el {
                Element::SectionStart(n) => writeln!(f, "<section {n}>")?,
                Element::SectionEnd => writeln!(f, "</section>")?,
                Element::KeyValue(k, v) => {
                    match String::from_utf8(v.clone()) {
                        Ok(s) => writeln!(f, "{k} = {s}")?,
                        Err(_) => writeln!(f, "{k} = 0x{}", hex(v))?,
                    }
                }
                Element::ListStart(n) => writeln!(f, "<list {n}>")?,
                Element::ListItem(v) => {
                    match String::from_utf8(v.clone()) {
                        Ok(s) => writeln!(f, "- {s}")?,
                        Err(_) => writeln!(f, "- 0x{}", hex(v))?,
                    }
                }
                Element::ListEnd => writeln!(f, "</list>")?,
            }
        }
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_simple_message() {
        let msg = Message::new()
            .section_start("root")
            .kv_str("key", "value")
            .list_start("ids")
            .list_item_str("a")
            .list_item_str("b")
            .list_end()
            .section_end();

        let encoded = msg.encode().unwrap();
        let decoded = Message::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }
}
