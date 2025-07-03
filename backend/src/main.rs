use std::{cmp::Ordering, env, net::TcpStream};
use tracing::{Level, debug, error, info, trace, warn};
use utils::{receive_data, send_data};

const DNS_IP: &str = "0.0.0.0:6202";
const CACHER_IP: &str = "0.0.0.0:6203";
const PTCL_VER: (u32, u32, u32) = (0, 0, 0);

fn main() {
    let mut dns_ip = String::from(DNS_IP);
    let mut cacher_ip = String::from(CACHER_IP);
    let mut verbose_level = 0u8;
    let mut dest_fqdn = String::new();
    let args: Vec<String> = env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                "dns-provider" => dns_ip = args[i + 1].clone(),
                "dns-cacher" => cacher_ip = args[i + 1].clone(),
                "resolve" => dest_fqdn = args[i + 1].clone(),
                _ => panic!("Pre-init failure; unknown long-name argument: {}", arg),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    'd' => {
                        dns_ip = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'c' => {
                        dns_ip = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'r' => {
                        dest_fqdn = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    _ => panic!("Pre-init failure; unknown short-name argument: {}", arg),
                }
            }
        }
    }
    let log_level = match verbose_level {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap_or_else(|_| {
        tracing_subscriber::fmt().init();
    });
    let mut fqdn: (Option<String>, Option<String>) = (None, None);
    'breakable: {
        if cacher_ip != String::new() {
            trace!("Contacting DNS Cacher {}", cacher_ip);
            let Ok(stream) = TcpStream::connect(&cacher_ip) else {
                warn!("Failed to contact DNS Cacher {}!", cacher_ip);
                break 'breakable;
            };
            info!("Connected to {}", cacher_ip);
            debug!("Locating {}", dest_fqdn);
            let dest_ip = cache_resolve(&stream, &dest_fqdn, &cacher_ip);
            info!(
                "Resolved {} to {}!",
                dest_fqdn,
                dest_ip.clone().unwrap_or_default()
            );
            if dest_ip != Some(String::new()) {
                fqdn.1 = dest_ip;
            }
        }
    }
    'breakable: {
        if dns_ip != String::new() {
            trace!("Attempting to resolve DNS Server {}", dns_ip);
            let Ok(stream) = TcpStream::connect(&dns_ip) else {
                warn!("Failed to resolve to DNS Server {}!", dns_ip);
                break 'breakable;
            };
            info!("Connected to {}", dns_ip);
            debug!("Attempting to resolve {}", dest_fqdn);
            let dest_ip = dns_resolve(&stream, &dest_fqdn, "", &dns_ip);
            info!(
                "Resolved {} to {}!",
                dest_fqdn,
                dest_ip.clone().unwrap_or_default()
            );
            if dest_ip != Some(String::new()) {
                fqdn.0 = dest_ip;
            }
        }
    }
    if fqdn.0.is_none() && fqdn.1.is_none() {
        error!(
            "IP lookup failed! Neither DNS Server nor DNS Cacher could locate {}",
            dest_fqdn
        );
    } else if fqdn.0 != fqdn.1 {
        error!(
            "IP mismatch! While resolving {}, DNS Server resolved {}, but DNS Cacher resolved {}!",
            dest_fqdn,
            fqdn.0.unwrap_or("None".to_owned()),
            fqdn.1.unwrap_or("None".to_owned())
        );
    } else {
        info!(
            "Both DNS Server and DNS Cacher resolved {} to {}.",
            dest_fqdn,
            fqdn.0.unwrap_or_default()
        );
    }
}

fn dns_resolve(stream: &TcpStream, destination: &str, prev: &str, dns_ip: &str) -> Option<String> {
    let block = destination.split('.').next_back().unwrap_or_default();
    let next_prev = ".".to_owned() + block + prev;
    let is_last_block = block == destination;
    let mut payload = PTCL_VER.0.to_le_bytes().to_vec();
    payload.extend_from_slice(&PTCL_VER.1.to_le_bytes());
    payload.extend_from_slice(if is_last_block { &[0u8] } else { &[1u8] });
    payload.extend_from_slice(block.as_bytes());
    send_data(&payload, stream);
    let response = receive_data(stream);
    match response.len().cmp(&4) {
        Ordering::Less => {
            error!("Server send an invalid response.");
            return None;
        }
        Ordering::Equal => {
            decode_error(&response.try_into().unwrap_or_default());
            return None;
        }
        _ => {}
    }
    match u32::from_le_bytes(response[0..4].try_into().unwrap()) {
        200 => {
            // 200: Server OK / Success
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("DNS resolved to destination {} early.", fqdn);
            }
            return Some(fqdn.into_owned());
        }
        203 => {
            // 203: Non-Authoritative Information
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!(
                "DNS fallback configured to correct FQN {} where doesn't exist.",
                fqdn
            );
            return Some(fqdn.into_owned());
        }
        301 => {
            // 301: Permenant Redirect
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!("DNS Server {} has moved to {}!", dns_ip, fqdn);
            trace!("Attempting to connect to new DNS Server {}", fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve new DNS Server {}!", fqdn);
                return None;
            };
            info!("Connected to {}", fqdn);
            return dns_resolve(&newstream, destination, prev, &fqdn);
        }
        302 => {
            // 302: Found
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if is_last_block {
                warn!(
                    "End of client chain reached, but server returned {} as DNS.",
                    fqdn
                );
            }
            trace!("Attempting to resolve intermediary DNS Server {}", &fqdn);
            let newdestination = if !is_last_block {
                destination
                    .rsplit('.')
                    .skip(1)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(".")
            } else {
                destination.to_string()
            };
            debug!("Passing {} to {}", newdestination, &fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve intermediary DNS {}!", fqdn);
                return None;
            };
            debug!("Attempting to resolve {}", newdestination);
            return dns_resolve(&newstream, &newdestination, &next_prev, &fqdn);
        }
        410 => {
            // 410: Client Error Gone
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("Reached end of DNS chain early! Rectifying FQN as {}", fqdn);
            }
            return Some(fqdn.into_owned());
        }
        421 => {
            // 421: Misdirected Request
            error!("DNS Server couldn't resolve {}.", next_prev);
        }
        _ => {
            error!(
                "DNS Server failure. Are you sure {} is a DNS Server?",
                destination
            );
        }
    }
    None
}

fn cache_resolve(stream: &TcpStream, destination: &str, dns_ip: &str) -> Option<String> {
    let mut payload = PTCL_VER.0.to_le_bytes().to_vec();
    payload.extend_from_slice(&PTCL_VER.1.to_le_bytes());
    payload.extend_from_slice(destination.as_bytes());
    send_data(&payload, stream);
    let response = receive_data(stream);
    match response.len().cmp(&4) {
        Ordering::Less => {
            error!("Server send an invalid response.");
            return None;
        }
        Ordering::Equal => {
            decode_error(&response.try_into().unwrap_or_default());
            return None;
        }
        _ => {}
    }
    match u32::from_le_bytes(response[0..4].try_into().unwrap()) {
        200 => {
            // 200: Server OK / Success
            let fqdn = String::from_utf8_lossy(&response[4..]);
            return Some(fqdn.into_owned());
        }
        301 => {
            // 301: Permenant Redirect
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!("DNS Cacher {} has moved to {}!", dns_ip, fqdn);
            trace!("Attempting to connect to new DNS Cacher {}", fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve new DNS Cacher {}!", fqdn);
                return None;
            };
            info!("Connected to {}", fqdn);
            return cache_resolve(&newstream, destination, &fqdn);
        }
        421 => {
            // 421: Misdirected Request
            error!("DNS Cacher couldn't resolve {}.", destination);
        }
        _ => {
            error!(
                "DNS Cacher failure. Are you sure {} is a DNS Cacher?",
                destination
            );
        }
    }
    None
}

fn decode_error(response: &[u8; 4]) {
    match u32::from_le_bytes(*response) {
        400 => error!("Bad request."),
        402 => error!("Payload too small."),
        426 => error!("Upgrade required."),
        427 => error!("Downgrade required."),
        _ => error!("Communication fault."),
    }
}
