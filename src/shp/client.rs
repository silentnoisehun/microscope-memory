//! SHP Client — connect to an SHP server for testing and CLI use.

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::genome;
use super::protocol::*;

pub struct ShpClient {
    stream: TcpStream,
}

impl ShpClient {
    pub async fn connect(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(ShpClient { stream })
    }

    async fn request(&mut self, cmd: Command, payload: &[u8]) -> std::io::Result<(Status, Vec<u8>)> {
        let hdr = RequestHeader {
            cmd,
            payload_len: payload.len() as u32,
            genome_hash: genome::genome_hash().hash,
        };
        self.stream.write_all(&hdr.to_bytes()).await?;
        if !payload.is_empty() {
            self.stream.write_all(payload).await?;
        }

        let mut hdr_buf = [0u8; SHP_HEADER_SIZE];
        self.stream.read_exact(&mut hdr_buf).await?;
        let resp = ResponseHeader::from_bytes(&hdr_buf)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad response magic"))?;

        let mut resp_payload = vec![0u8; resp.payload_len as usize];
        if resp.payload_len > 0 {
            self.stream.read_exact(&mut resp_payload).await?;
        }

        Ok((resp.status, resp_payload))
    }

    pub async fn ping(&mut self) -> std::io::Result<bool> {
        let (status, payload) = self.request(Command::Ping, &[]).await?;
        Ok(status == Status::Ok && payload == b"PONG")
    }

    pub async fn store(&mut self, text: &str, layer_id: u8, importance: u8) -> std::io::Result<Status> {
        let payload = encode_store(layer_id, importance, text);
        let (status, _) = self.request(Command::Store, &payload).await?;
        Ok(status)
    }

    pub async fn recall(&mut self, query: &str, k: u16) -> std::io::Result<Vec<ResultEntry>> {
        let payload = encode_query(query, k);
        let (status, resp) = self.request(Command::Recall, &payload).await?;
        if status != Status::Ok { return Ok(Vec::new()); }
        Ok(decode_results(&resp))
    }

    pub async fn look(&mut self, x: f32, y: f32, z: f32, zoom: u8, k: u16) -> std::io::Result<Vec<ResultEntry>> {
        let payload = encode_look(x, y, z, zoom, k);
        let (status, resp) = self.request(Command::Look, &payload).await?;
        if status != Status::Ok { return Ok(Vec::new()); }
        Ok(decode_results(&resp))
    }

    pub async fn find(&mut self, query: &str, k: u16) -> std::io::Result<Vec<ResultEntry>> {
        let payload = encode_query(query, k);
        let (status, resp) = self.request(Command::Find, &payload).await?;
        if status != Status::Ok { return Ok(Vec::new()); }
        Ok(decode_results(&resp))
    }

    pub async fn teach(&mut self, query: &str, response: &str) -> std::io::Result<(Status, String)> {
        let payload = encode_teach(query, response);
        let (status, resp) = self.request(Command::Teach, &payload).await?;
        let msg = String::from_utf8_lossy(&resp).into_owned();
        Ok((status, msg))
    }

    pub async fn verify(&mut self, target: u8) -> std::io::Result<(bool, String)> {
        let (status, resp) = self.request(Command::Verify, &[target]).await?;
        let msg = String::from_utf8_lossy(&resp).into_owned();
        Ok((status == Status::Ok, msg))
    }

    pub async fn stats(&mut self) -> std::io::Result<(Status, Vec<u8>)> {
        self.request(Command::Stats, &[]).await
    }

    /// LookPacket: returns raw SHP v1.0 packets (372 bytes each, zero-copy format).
    pub async fn look_packet(&mut self, x: f32, y: f32, z: f32, zoom: u8, k: u16) -> std::io::Result<Vec<super::packet::ShpPacket>> {
        let payload = encode_look(x, y, z, zoom, k);
        let (status, resp) = self.request(Command::LookPacket, &payload).await?;
        if status != Status::Ok { return Ok(Vec::new()); }
        if resp.len() < 2 { return Ok(Vec::new()); }

        let count = u16::from_le_bytes([resp[0], resp[1]]) as usize;
        let mut packets = Vec::with_capacity(count);
        let mut off = 2;
        for _ in 0..count {
            if off + super::packet::SHP_PACKET_SIZE > resp.len() { break; }
            let mut buf = [0u8; super::packet::SHP_PACKET_SIZE];
            buf.copy_from_slice(&resp[off..off + super::packet::SHP_PACKET_SIZE]);
            if let Some(pkt) = super::packet::ShpPacket::from_bytes(&buf) {
                packets.push(*pkt);
            }
            off += super::packet::SHP_PACKET_SIZE;
        }
        Ok(packets)
    }
}
