use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u8, actual: u8 },

    #[error("Message too large: {size} bytes")]
    MessageTooLarge { size: u32 },
}

pub struct Protocol {
    version: u8,
    max_message_size: u32,
}

impl Protocol {
    pub fn new() -> Self {
        Self {
            version: PROTOCOL_VERSION,
            max_message_size: 1024 * 1024, // 1MB default
        }
    }

    pub fn write_message<W: Write, M: Serialize>(
        &self,
        writer: &mut W,
        message: &M,
    ) -> Result<(), ProtocolError> {
        // Serialize message
        let data = bincode::serialize(message)?;

        // Check message size
        let len = data.len() as u32;
        if len > self.max_message_size {
            return Err(ProtocolError::MessageTooLarge { size: len });
        }

        // Write header: version (1 byte) + length (4 bytes)
        writer.write_all(&[self.version])?;
        writer.write_all(&len.to_be_bytes())?;

        // Write payload
        writer.write_all(&data)?;
        writer.flush()?;

        Ok(())
    }

    pub fn read_message<R: Read, M: for<'de> Deserialize<'de>>(
        &self,
        reader: &mut R,
    ) -> Result<M, ProtocolError> {
        // Read and verify version
        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;
        if version[0] != self.version {
            return Err(ProtocolError::VersionMismatch {
                expected: self.version,
                actual: version[0],
            });
        }

        // Read length
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes);

        // Check message size
        if len > self.max_message_size {
            return Err(ProtocolError::MessageTooLarge { size: len });
        }

        // Read payload
        let mut data = vec![0u8; len as usize];
        reader.read_exact(&mut data)?;

        // Deserialize
        Ok(bincode::deserialize(&data)?)
    }
}
