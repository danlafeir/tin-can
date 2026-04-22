use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use anyhow::{Context, Result};
use str0m::Candidate;
use tracing::{debug, warn};

const STUN_SERVER: &str = "stun.l.google.com:19302";
const STUN_TIMEOUT: Duration = Duration::from_secs(3);

/// Bind a UDP socket and gather host + optional STUN server-reflexive candidates.
/// Returns the socket, the real local address (for use in Receive::new), and candidates.
pub fn gather() -> Result<(UdpSocket, SocketAddr, Vec<Candidate>)> {
    let socket = UdpSocket::bind("0.0.0.0:0").context("bind UDP socket")?;
    let port = socket.local_addr().context("get local addr")?.port();

    // Discover the outbound interface IP — connecting a UDP socket doesn't
    // send any packets, but causes the OS to resolve the route and fill in
    // the source address.
    let local_ip = {
        let probe = UdpSocket::bind("0.0.0.0:0").context("bind probe socket")?;
        probe.connect("8.8.8.8:80").context("connect probe")?;
        probe.local_addr().context("probe local addr")?.ip()
    };
    let local_addr = SocketAddr::new(local_ip, port);
    debug!("bound UDP socket on {}", local_addr);

    let mut candidates = Vec::new();

    // Host candidate — works on same LAN / loopback
    let host = Candidate::host(local_addr, "udp")
        .context("create host candidate")?;
    candidates.push(host);

    // Server-reflexive candidate via STUN — needed to punch through NAT
    match query_stun(&socket, local_addr) {
        Ok(public_addr) if public_addr != local_addr => {
            debug!("STUN mapped address: {}", public_addr);
            match Candidate::server_reflexive(public_addr, local_addr, "udp") {
                Ok(srflx) => candidates.push(srflx),
                Err(e) => warn!("failed to create srflx candidate: {}", e),
            }
        }
        Ok(_) => debug!("STUN returned same address as local — no NAT"),
        Err(e) => warn!("STUN query failed (will use host candidates only): {}", e),
    }

    // Restore blocking mode after STUN query modified the timeout
    socket.set_read_timeout(None).ok();

    Ok((socket, local_addr, candidates))
}

/// Sends a minimal STUN Binding Request and parses the XOR-MAPPED-ADDRESS from the response.
fn query_stun(socket: &UdpSocket, local_addr: SocketAddr) -> Result<SocketAddr> {
    let stun_addr: SocketAddr = {
        use std::net::ToSocketAddrs;
        STUN_SERVER
            .to_socket_addrs()
            .context("resolve STUN server")?
            .find(|a| a.is_ipv4() == local_addr.is_ipv4())
            .context("no matching STUN address")?
    };

    // STUN Binding Request — 20 byte header, no attributes
    let mut req = [0u8; 20];
    req[0] = 0x00;
    req[1] = 0x01; // Message Type: Binding Request
    // Message Length = 0 (bytes 2-3 remain zero)
    req[4] = 0x21;
    req[5] = 0x12;
    req[6] = 0xA4;
    req[7] = 0x42; // Magic Cookie
    // Transaction ID: bytes 8-19 (12 bytes), use simple fixed value
    req[8..20].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

    socket
        .send_to(&req, stun_addr)
        .context("send STUN request")?;

    let mut buf = [0u8; 512];
    socket
        .set_read_timeout(Some(STUN_TIMEOUT))
        .context("set STUN recv timeout")?;
    let (n, _) = socket.recv_from(&mut buf).context("receive STUN response")?;

    parse_stun_mapped_address(&buf[..n], local_addr.is_ipv4())
        .context("parse STUN XOR-MAPPED-ADDRESS")
}

/// Parses an XOR-MAPPED-ADDRESS attribute (type 0x0020) from a STUN response.
fn parse_stun_mapped_address(data: &[u8], expect_ipv4: bool) -> Result<SocketAddr> {
    if data.len() < 20 {
        anyhow::bail!("STUN response too short");
    }

    // Verify it's a Binding Success Response (0x0101)
    if data[0] != 0x01 || data[1] != 0x01 {
        anyhow::bail!(
            "unexpected STUN message type: 0x{:02x}{:02x}",
            data[0],
            data[1]
        );
    }

    let magic: [u8; 4] = [0x21, 0x12, 0xA4, 0x42];
    let mut offset = 20; // skip header

    while offset + 4 <= data.len() {
        let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if attr_type == 0x0020 {
            // XOR-MAPPED-ADDRESS
            if offset + attr_len > data.len() {
                anyhow::bail!("XOR-MAPPED-ADDRESS truncated");
            }
            let family = data[offset + 1];
            let xor_port = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
            let port = xor_port ^ 0x2112;

            if family == 0x01 && expect_ipv4 {
                // IPv4: XOR with magic cookie
                let xor_addr = [
                    data[offset + 4] ^ magic[0],
                    data[offset + 5] ^ magic[1],
                    data[offset + 6] ^ magic[2],
                    data[offset + 7] ^ magic[3],
                ];
                return Ok(SocketAddr::from((xor_addr, port)));
            } else if family == 0x02 && !expect_ipv4 {
                // IPv6: XOR with magic cookie + transaction ID
                let mut xor_addr = [0u8; 16];
                for i in 0..16 {
                    let xor_byte = if i < 4 { magic[i] } else { data[8 + (i - 4)] };
                    xor_addr[i] = data[offset + 4 + i] ^ xor_byte;
                }
                let addr: std::net::Ipv6Addr = xor_addr.into();
                return Ok(SocketAddr::from((addr, port)));
            }
        }

        // Attributes are padded to 4-byte boundaries
        let padded = (attr_len + 3) & !3;
        offset += padded;
    }

    anyhow::bail!("XOR-MAPPED-ADDRESS not found in STUN response")
}

