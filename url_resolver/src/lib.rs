use async_std::task;
use futures::{FutureExt, select};
use std::{cmp::Ordering, env, net::TcpStream};
use tracing::{debug, error, info, trace, warn};
use utils::{receive_data, send_data, status};

const DNS_IP: &str = "0.0.0.0:6202";
const CACHER_IP: &str = "0.0.0.0:6203";

pub async fn resolve(
    dest_addr: &str,
    integrity_check: Option<bool>,
    dns_ip: Option<&str>,
    cacher_ip: Option<&str>,
) -> String {
    let program_version: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|f| match f.parse::<u32>() {
            Ok(version) => version,
            Err(e) => {
                panic!("Failed to parse version: {e}");
            }
        })
        .collect();
    assert!(program_version.len() > 2);
    let dns_ip = dns_ip.unwrap_or(DNS_IP).to_owned();
    let cacher_ip = cacher_ip.unwrap_or(CACHER_IP).to_owned();
    let integrity_check = integrity_check.unwrap_or(false);
    let mut result = String::new();
    let dest_addr_clone = dest_addr.to_owned();
    let mut cache_handle =
        task::spawn(async move { cache_task(&cacher_ip, &dest_addr_clone).await }).fuse();
    let dest_addr_clone = dest_addr.to_owned();
    let mut dns_handle =
        task::spawn(async move { dns_task(&dns_ip, &dest_addr_clone).await }).fuse();
    let mut comparison = None;
    let data = select! {
        result = cache_handle => {
            let mut return_data = None;
            if result.is_some() {
                info!("Cache handle returned first");
                return_data = result.clone();
                if integrity_check {comparison = Some(task::spawn(compare_results(result.unwrap().0, dns_handle)));}
            }
            else {
                warn!("Cache handle returned None! Fallback to DNS handle.");
                let dns_res = dns_handle.await;
                if dns_res.is_some() {
                    return_data = dns_res;
                }
                else {
                    warn!("Unable to resolve {}!", dest_addr);
                }
            }
            return_data
        }
        result = dns_handle => {
            let mut return_data = None;
            if result.is_some() {
                info!("DNS handle returned first");
                return_data = result.clone();
                if integrity_check {comparison = Some(task::spawn(compare_results(result.unwrap().0, cache_handle)));}
            }
            else {
                warn!("DNS handle returned None! Fallback to cache handle.");
                let cache_res = cache_handle.await;
                if cache_res.is_some() {
                    return_data = cache_res;
                }
                else {
                    warn!("Unable to resolve {}!", dest_addr);
                }
            }
            return_data
        }
    };
    if data.is_some() {
        let response = data.unwrap();
        result = format!("{}/{}", response.0, response.1);
    } else {
        error!("Unable to resolve {}.", dest_addr);
    }
    if comparison.is_some() && integrity_check {
        comparison.unwrap().await;
    }
    result
}

fn address_splitter(address: &str) -> (String, String) {
    let addr = address.strip_prefix("web://").unwrap_or(address);
    let (fqdn, endpoint) = addr.split_once('/').unwrap_or((addr, ""));
    (fqdn.to_string(), endpoint.to_string())
}

async fn dns_task(dns_ip: &str, dest_addr: &str) -> Option<(String, String)> {
    let (dest_fqdn, dest_endpoint) = address_splitter(dest_addr);
    if dns_ip != String::new() {
        trace!("Attempting to resolve DNS Server {}", dns_ip);
        let Ok(stream) = TcpStream::connect(dns_ip) else {
            warn!("Failed to resolve to DNS Server {}!", dns_ip);
            return None;
        };
        info!("Connected to {}", dns_ip);
        debug!("Attempting to resolve {}", dest_fqdn);
        let dest_ip = dns_resolve(&stream, &dest_fqdn, "", dns_ip, &["".to_string()]);
        info!(
            "Resolved {} to {}!",
            dest_fqdn,
            dest_ip.clone().unwrap_or_default()
        );
        if let Some(dest_ip) = dest_ip {
            return if dest_ip == String::new() {
                None
            } else {
                Some((dest_ip, dest_endpoint))
            };
        }
    }
    None
}

fn dns_resolve(
    stream: &TcpStream,
    destination: &str,
    prev: &str,
    dns_ip: &str,
    routes: &[String],
) -> Option<String> {
    let block = destination.split('.').next_back().unwrap_or_default();
    let next_prev = ".".to_owned() + block + prev;
    let is_last_block = block == destination;
    let version: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|f| f.parse::<u32>().unwrap())
        .collect();
    let mut payload = version[0].to_le_bytes().to_vec();
    payload.extend_from_slice(&version[1].to_le_bytes());
    payload.extend_from_slice(&version[2].to_le_bytes());
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
        status::SUCCESS => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("DNS resolved to destination {} early.", fqdn);
            }
            return Some(fqdn.into_owned());
        }
        status::NON_AUTHOROTATIVE => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!(
                "DNS fallback configured to correct FQN {} where doesn't exist.",
                fqdn
            );
            return Some(fqdn.into_owned());
        }
        status::PERMANENT_REDIRECT => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!("DNS Server {} has moved to {}!", dns_ip, fqdn);
            trace!("Attempting to connect to new DNS Server {}", fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve new DNS Server {}!", fqdn);
                return None;
            };
            info!("Connected to {}", fqdn);
            return dns_resolve(&newstream, destination, prev, &fqdn, routes);
        }
        status::FOUND => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if is_last_block {
                warn!(
                    "End of client chain reached, but server returned {} as DNS.",
                    fqdn
                );
                if routes.contains(&fqdn.to_string()) {
                    error!(
                        "DNS redirection has looped. Please notify DNS provider of misconfiguration."
                    );
                    return None;
                }
            }
            let mut routes = routes.to_vec();
            routes.push(fqdn.to_string());
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
            return dns_resolve(&newstream, &newdestination, &next_prev, &fqdn, &routes);
        }
        status::CLIENT_ERROR_GONE => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("Reached end of DNS chain early! Rectifying FQN as {}", fqdn);
            }
            return Some(fqdn.into_owned());
        }
        status::MISDIRECTED => {
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

async fn cache_task(cacher_ip: &str, dest_addr: &str) -> Option<(String, String)> {
    let (dest_fqdn, dest_endpoint) = address_splitter(dest_addr);
    if cacher_ip != String::new() {
        trace!("Contacting DNS Cacher {}", cacher_ip);
        let Ok(stream) = TcpStream::connect(cacher_ip) else {
            warn!("Failed to contact DNS Cacher {}!", cacher_ip);
            return None;
        };
        info!("Connected to {}", cacher_ip);
        debug!("Locating {}", dest_fqdn);
        let dest_ip = cache_resolve(&stream, &dest_fqdn, cacher_ip);
        info!(
            "Resolved {} to {}!",
            dest_fqdn,
            dest_ip.clone().unwrap_or_default()
        );
        if let Some(dest_ip) = dest_ip {
            return if dest_ip == String::new() {
                None
            } else {
                Some((dest_ip, dest_endpoint))
            };
        }
    }
    None
}

fn cache_resolve(stream: &TcpStream, destination: &str, dns_ip: &str) -> Option<String> {
    let version: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|f| f.parse::<u32>().unwrap())
        .collect();
    let mut payload = version[0].to_le_bytes().to_vec();
    payload.extend_from_slice(&version[1].to_le_bytes());
    payload.extend_from_slice(&version[2].to_le_bytes());
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
        status::SUCCESS => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            return Some(fqdn.into_owned());
        }
        status::PERMANENT_REDIRECT => {
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
        status::MISDIRECTED => {
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

// async fn get_data(address: (String, String), stacks: &str) -> Option<Vec<u8>> {
//     let Ok(stream) = TcpStream::connect(&address.0) else {
//         error!("Failed to connect to {}!", &address.0);
//         return None;
//     };
//     let mut payload = PTCL_VER.0.to_le_bytes().to_vec();
//     payload.extend_from_slice(&PTCL_VER.1.to_le_bytes());
//     payload.extend_from_slice(&PTCL_VER.2.to_le_bytes());
//     payload.extend_from_slice(stacks.as_bytes());
//     payload.extend_from_slice("/".as_bytes());
//     payload.extend_from_slice(address.1.as_bytes());
//     send_data(&payload, &stream);
//     let response = receive_data(&stream);
//     match response.len() {
//         4 => {
//             decode_error(&response.try_into().unwrap_or_default());
//         }
//         0..9 => {
//             error!("Server send an invalid response.");
//             return None;
//         }
//         _ => {
//             match u32::from_le_bytes(response[0..4].try_into().unwrap()) {
//                 status::SUCCESS => {
//                     println!(
//                         "Server responsed with protocol {}",
//                         String::from_utf8_lossy(&response[4..9])
//                     );
//                     return Some(response[9..].to_vec());
//                 }
//                 100 => {
//                     // Handle extra blocks here
//                 }
//                 _ => {
//                     error!("Server send an invalid response.");
//                     return None;
//                 }
//             }
//         }
//     }
//     // Some(receive_data(&stream))
//     None
// }

async fn compare_results(
    complete: String,
    future: futures::future::Fuse<task::JoinHandle<Option<(String, String)>>>,
) {
    let result = future.await;
    if result.is_some() && result.unwrap().0 != complete {
        error!("DNS Server and DNS Cacher returned different results!");
        //report to DNS cacher that its information is outdated
    }
}

fn decode_error(response: &[u8; 4]) {
    match u32::from_le_bytes(*response) {
        status::TEST_NOT_IMPLEMENTED => error!("[TESTING] Not implemented."),
        status::BAD_REQUEST => error!("Bad request."),
        status::TOO_SMALL => error!("Payload too small."),
        status::FORBIDDEN => error!("Forbidden action."),
        status::NOT_FOUND => error!("Resource not found."),
        status::UNPROCESSABLE => error!("Unprocessable request."),
        status::UPGRADE_REQUIRED => error!("Upgrade required."),
        status::DOWNGRADE_REQUIRED => error!("Downgrade required."),
        status::NOT_IMPLEMENTED => error!("Operation not implemented."),
        _ => error!("Communication fault."),
    }
}
