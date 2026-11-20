use crate::config::DeviceInfo;
use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u32 = 1;
pub const FRAME_CONTROL: u8 = 1;
pub const FRAME_CHUNK: u8 = 2;
pub const MAX_CONTROL_FRAME: usize = 256 * 1024;
pub const MAX_CHUNK_FRAME: usize = 1024 * 1024 + 16;
pub const CHUNK_SIZE: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Control {
    ClientHello {
        protocol: u32,
        request_id: String,
        device: DeviceInfo,
        tcp_port: u16,
    },
    ServerHello {
        protocol: u32,
        accepted: bool,
        session_id: Option<String>,
        device: Option<DeviceInfo>,
        reason: Option<String>,
    },
    Text {
        id: String,
        text: String,
        at: i64,
    },
    FileOffer {
        id: String,
        seq: u64,
        name: String,
        size: u64,
    },
    FileAccept {
        id: String,
        offset: u64,
    },
    FileReject {
        id: String,
        reason: String,
    },
    FileComplete {
        id: String,
    },
    FileReceived {
        id: String,
    },
    TransferFailed {
        id: String,
        reason: String,
    },
    PauseRequest {
        id: String,
    },
    ResumeRequest {
        id: String,
    },
    TransferPaused {
        id: String,
    },
    TransferResumed {
        id: String,
    },
    TransferCancel {
        id: String,
    },
}

pub fn encode_frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 5);
    frame.extend_from_slice(&((payload.len() + 1) as u32).to_be_bytes());
    frame.push(kind);
    frame.extend_from_slice(payload);
    frame
}

pub fn encode_control(message: &Control) -> Vec<u8> {
    let payload = serde_json::to_vec(message).expect("control messages are serializable");
    encode_frame(FRAME_CONTROL, &payload)
}

pub fn encode_chunk(seq: u64, offset: u64, data: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(data.len() + 21);
    frame.extend_from_slice(&((data.len() + 17) as u32).to_be_bytes());
    frame.push(FRAME_CHUNK);
    frame.extend_from_slice(&seq.to_be_bytes());
    frame.extend_from_slice(&offset.to_be_bytes());
    frame.extend_from_slice(data);
    frame
}

pub async fn write_control<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &Control,
) -> io::Result<()> {
    writer.write_all(&encode_control(message)).await
}

pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_payload: usize,
) -> io::Result<(u8, Vec<u8>)> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length).await?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > max_payload {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }

    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    let kind = payload.remove(0);
    Ok((kind, payload))
}

pub async fn read_control<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Control> {
    let (kind, payload) = read_frame(reader, MAX_CONTROL_FRAME).await?;
    if kind != FRAME_CONTROL {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected a control frame",
        ));
    }
    serde_json::from_slice(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// LAN-only guard: we never want the transfer listener exposed to the open
/// internet, even when the machine has a public address.
pub fn is_lan_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
        }
        std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_fields_use_camel_case() {
        let hello = Control::ClientHello {
            protocol: 1,
            request_id: "request".to_string(),
            device: DeviceInfo {
                id: "id".to_string(),
                name: "name".to_string(),
                platform: "macos".to_string(),
            },
            tcp_port: 1234,
        };
        let json = serde_json::to_string(&hello).unwrap();
        assert!(json.contains("\"requestId\""));
        assert!(json.contains("\"tcpPort\""));
        assert!(!json.contains("request_id"));
    }
}
