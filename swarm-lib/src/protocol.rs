use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

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

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Connection failed after {attempts} attempts")]
    MaxRetriesExceeded { attempts: u32 },

    #[error("Operation timed out after {duration:?}")]
    Timeout { duration: Duration },
}

pub struct Connection {
    addr: String,
    stream: Option<TcpStream>,
    protocol: Protocol,
    max_retries: u32,
    timeout: Duration,
    base_retry_delay: Duration,
}

impl Connection {
    pub fn new<A: ToSocketAddrs>(
        addr: A,
        max_retries: u32,
        timeout: Duration,
    ) -> Result<Self, ConnectionError> {
        Ok(Self {
            addr: addr
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Invalid address",
                    )
                })?
                .to_string(),
            stream: None,
            protocol: Protocol::new(),
            max_retries,
            timeout,
            base_retry_delay: Duration::from_millis(100),
        })
    }

    fn ensure_connected(&mut self) -> Result<(), ConnectionError> {
        if self.stream.is_some() {
            return Ok(());
        }

        let mut attempts = 0;
        let mut delay = self.base_retry_delay;

        while attempts < self.max_retries {
            match TcpStream::connect(&self.addr) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(self.timeout))?;
                    self.stream = Some(stream);
                    return Ok(());
                }
                Err(_) => {
                    attempts += 1;
                    if attempts == self.max_retries {
                        return Err(ConnectionError::MaxRetriesExceeded {
                            attempts,
                        });
                    }
                    std::thread::sleep(delay);
                    delay = delay.saturating_mul(2); // exponential backoff
                }
            }
        }

        Err(ConnectionError::MaxRetriesExceeded { attempts })
    }

    pub fn send<M: Serialize>(
        &mut self,
        message: &M,
    ) -> Result<(), ConnectionError> {
        let start = Instant::now();
        let mut attempts = 0;

        while attempts < self.max_retries {
            if start.elapsed() > self.timeout {
                return Err(ConnectionError::Timeout {
                    duration: start.elapsed(),
                });
            }

            self.ensure_connected()?;
            let stream = self.stream.as_mut().unwrap();

            match self.protocol.write_message(stream, message) {
                Ok(()) => return Ok(()),
                Err(_) => {
                    self.stream = None; // Force reconnect on next attempt
                    attempts += 1;
                }
            }
        }

        Err(ConnectionError::MaxRetriesExceeded { attempts })
    }

    pub fn receive<M: for<'de> Deserialize<'de>>(
        &mut self,
    ) -> Result<M, ConnectionError> {
        let start = Instant::now();
        let mut attempts = 0;

        while attempts < self.max_retries {
            if start.elapsed() > self.timeout {
                return Err(ConnectionError::Timeout {
                    duration: start.elapsed(),
                });
            }

            self.ensure_connected()?;
            let stream = self.stream.as_mut().unwrap();

            match self.protocol.read_message(stream) {
                Ok(msg) => return Ok(msg),
                Err(_) => {
                    self.stream = None; // Force reconnect on next attempt
                    attempts += 1;
                }
            }
        }

        Err(ConnectionError::MaxRetriesExceeded { attempts })
    }
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
