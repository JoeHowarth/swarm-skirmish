use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_MESSAGE_SIZE: u32 = 1024 * 1024; // 1MB default

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u8, actual: u8 },

    #[error("Message too large: {size} bytes")]
    MessageTooLarge { size: u32 },
}

// Convert from serde_json::Error to our ProtocolError
impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::Serialization(err.to_string())
    }
}

pub struct Protocol;

impl Protocol {
    pub fn write_message<W: Write, M: Serialize>(
        writer: &mut W,
        message: &M,
    ) -> Result<(), ProtocolError> {
        // Serialize message
        let data = serde_json::to_vec(message)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?;

        // Check message size
        let len = data.len() as u32;
        if len > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::MessageTooLarge { size: len });
        }

        // Write header: version (1 byte) + length (4 bytes)
        writer.write_all(&[PROTOCOL_VERSION])?;
        writer.write_all(&len.to_be_bytes())?;

        // Write payload
        writer.write_all(&data)?;
        writer.flush()?;

        Ok(())
    }

    pub fn read_message<R: Read, M: for<'de> Deserialize<'de>>(
        reader: &mut R,
    ) -> Result<M, ProtocolError> {
        // Read and verify version
        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;
        if version[0] != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                actual: version[0],
            });
        }

        // Read length
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes);

        // Check message size
        if len > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::MessageTooLarge { size: len });
        }

        // Read payload
        let mut data = vec![0u8; len as usize];
        reader.read_exact(&mut data)?;

        // Deserialize
        Ok(serde_json::from_slice(&data)
            .map_err(|e| ProtocolError::Serialization(e.to_string()))?)
    }
}
