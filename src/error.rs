//! Error types for rustici.
use std::{fmt, io, string::FromUtf8Error};

/// A convenient result alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors that may occur while encoding/decoding or doing IO.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O error.
    Io(io::Error),
    /// Protocol violation / unexpected packet type or invalid structure.
    Protocol(&'static str),
    /// The remote reported an unknown command.
    UnknownCommand(String),
    /// Too large message/field.
    TooLong(&'static str),
    /// UTF-8 conversion failed (when interpreting bytes as a String).
    Utf8(FromUtf8Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self { Error::Io(e) }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self { Error::Utf8(e) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Protocol(s) => write!(f, "protocol error: {s}"),
            Error::UnknownCommand(cmd) => write!(f, "unknown command: {cmd}"),
            Error::TooLong(what) => write!(f, "value too long: {what}"),
            Error::Utf8(e) => write!(f, "utf-8 error: {e}"),
        }
    }
}

impl std::error::Error for Error {}
