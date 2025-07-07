use std::{
    cmp::Ordering,
    io::{Read, Write},
    net::TcpStream,
};
use tracing::{debug, trace, warn};

pub fn receive_data(mut stream: &TcpStream) -> Vec<u8> {
    trace!("Started receiving data.");
    let mut len = [0; 2];
    let mut data = Vec::new();
    loop {
        match stream.read_exact(&mut len) {
            Ok(_) => {}
            Err(e) => {
                trace!("Failed to read block length: {}", e);
                return data;
            }
        }
        let len = u16::from_le_bytes(len);
        if len == 0 {
            trace!("Received null terminator.");
            break;
        }
        trace!("Expecting {len} bytes...");
        let start = data.len();
        data.extend(std::iter::repeat_n(0, len as usize));
        stream.read_exact(&mut data[start..]).unwrap();
        trace!("Received block of size {}.", data.len() - start);
        if len != u16::MAX {
            break;
        }
        trace!("Expecting another block...");
    }
    debug!("Finished receiving data of size {}", data.len());
    data
}

pub fn send_data(payload: &[u8], mut stream: &TcpStream) {
    debug!("Started sending data of size {}", payload.len());
    for block in payload.chunks(u16::MAX.into()) {
        let message_len = block.len() as u16;
        trace!("Announcing {message_len} bytes");
        match stream.write_all(&message_len.to_le_bytes()) {
            Ok(_) => {}
            Err(e) => {
                trace!("Failed to send block length: {}", e);
                return;
            }
        }
        trace!("Sending block of size {}...", block.len());
        match stream.write_all(block) {
            Ok(_) => {}
            Err(e) => {
                trace!("Failed to send block: {}", e);
                return;
            }
        }
    }
    if payload.len() % u16::MAX as usize == 0 {
        trace!("Sending null terminator");
        match stream.write_all(&0u16.to_le_bytes()) {
            Ok(_) => {}
            Err(e) => {
                trace!("Failed to send null terminator: {}", e);
            }
        }
    }
    trace!("Finished sending data.");
}

pub fn send_error(stream: &TcpStream, err: i32) {
    send_data(&err.to_le_bytes(), stream);
    stream
        .shutdown(std::net::Shutdown::Both)
        .unwrap_or_default();
}

pub fn version_compare(
    client: (u32, u32, u32),
    peer: std::net::SocketAddr,
    ptcl_ver: (u32, u32, u32),
) -> Ordering {
    match client.0.cmp(&ptcl_ver.0) {
        Ordering::Greater => {
            warn!(
                "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
                peer.ip(),
                peer.port(),
                client.0,
                client.1,
                ptcl_ver.0,
                ptcl_ver.1
            );
            return Ordering::Greater;
        }
        Ordering::Less => {
            warn!(
                "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
                peer.ip(),
                peer.port(),
                client.0,
                client.1,
                ptcl_ver.0,
                ptcl_ver.1
            );
            return Ordering::Less;
        }
        _ => match client.1.cmp(&ptcl_ver.1) {
            Ordering::Greater => {
                warn!(
                    "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
                    peer.ip(),
                    peer.port(),
                    client.0,
                    client.1,
                    ptcl_ver.0,
                    ptcl_ver.1
                );
                return Ordering::Greater;
            }
            Ordering::Less => {
                warn!(
                    "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
                    peer.ip(),
                    peer.port(),
                    client.0,
                    client.1,
                    ptcl_ver.0,
                    ptcl_ver.1
                );
                return Ordering::Less;
            }
            _ => {
                if client.2.cmp(&ptcl_ver.2) == Ordering::Less {
                    warn!(
                        "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
                        peer.ip(),
                        peer.port(),
                        client.0,
                        client.1,
                        ptcl_ver.0,
                        ptcl_ver.1
                    );
                    return Ordering::Less;
                }
            }
        },
    }
    Ordering::Equal
}
