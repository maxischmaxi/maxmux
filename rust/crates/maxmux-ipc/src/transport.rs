use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::UnixStream;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Connection closed")]
    Closed,
}

pub struct Connection {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
}

impl Connection {
    pub fn new(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            reader: BufReader::new(read),
            writer: BufWriter::new(write),
        }
    }

    /// Read one line and parse as the given message type
    pub async fn read_message<T: serde::de::DeserializeOwned>(
        &mut self,
    ) -> Result<T, TransportError> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(TransportError::Closed);
        }
        Ok(serde_json::from_str(line.trim())?)
    }

    /// Serialize a message and send as one line
    pub async fn send_message<T: serde::Serialize>(
        &mut self,
        msg: &T,
    ) -> Result<(), TransportError> {
        let json = serde_json::to_string(msg)?;
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// A split connection for use with tokio::select!
/// Allows reading and writing concurrently
pub struct SplitConnection {
    pub reader: MessageReader,
    pub writer: MessageWriter,
}

pub struct MessageReader {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
}

pub struct MessageWriter {
    writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
}

impl SplitConnection {
    pub fn new(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            reader: MessageReader {
                reader: BufReader::new(read),
            },
            writer: MessageWriter {
                writer: BufWriter::new(write),
            },
        }
    }
}

impl MessageReader {
    pub async fn read_message<T: serde::de::DeserializeOwned>(
        &mut self,
    ) -> Result<T, TransportError> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(TransportError::Closed);
        }
        Ok(serde_json::from_str(line.trim())?)
    }
}

impl MessageWriter {
    pub async fn send_message<T: serde::Serialize>(
        &mut self,
        msg: &T,
    ) -> Result<(), TransportError> {
        let json = serde_json::to_string(msg)?;
        self.writer.write_all(json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ClientMessage, ServerMessage};

    #[tokio::test]
    async fn test_connection_roundtrip() {
        let (s1, s2) = UnixStream::pair().unwrap();
        let mut conn1 = Connection::new(s1);
        let mut conn2 = Connection::new(s2);

        let msg = ClientMessage::Resize { cols: 80, rows: 24 };
        conn1.send_message(&msg).await.unwrap();
        let received: ClientMessage = conn2.read_message().await.unwrap();
        assert!(matches!(
            received,
            ClientMessage::Resize { cols: 80, rows: 24 }
        ));
    }

    #[tokio::test]
    async fn test_split_connection() {
        let (s1, s2) = UnixStream::pair().unwrap();
        let split1 = SplitConnection::new(s1);
        let mut split2 = SplitConnection::new(s2);

        let mut writer = split1.writer;
        let msg = ServerMessage::Error {
            message: "test error".into(),
        };
        writer.send_message(&msg).await.unwrap();
        let received: ServerMessage = split2.reader.read_message().await.unwrap();
        assert!(matches!(received, ServerMessage::Error { message } if message == "test error"));
    }

    #[tokio::test]
    async fn test_connection_closed() {
        let (s1, s2) = UnixStream::pair().unwrap();
        let mut conn2 = Connection::new(s2);
        drop(s1); // Close the other end
        let result: Result<ClientMessage, _> = conn2.read_message().await;
        assert!(result.is_err());
    }
}
