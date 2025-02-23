use std::{
    io,
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::{Protocol, ProtocolError};

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
