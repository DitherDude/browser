use async_std::task;
use futures::{FutureExt, select};
use std::{cmp::Ordering, env, net::TcpStream};
use tracing::{Level, debug, error, info, trace, warn};
use utils::{receive_data, send_data};

const DNS_IP: &str = "0.0.0.0:6202";
const CACHER_IP: &str = "0.0.0.0:6203";
const PTCL_VER: (u32, u32, u32) = (0, 0, 0);

#[async_std::main]
async fn main() {
    let mut dns_ip = String::from(DNS_IP);
    let mut cacher_ip = String::from(CACHER_IP);
    let mut verbose_level = 0u8;
    let mut dest_fqdn = String::new();
    let mut data_saver = false;
    let args: Vec<String> = env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "data-saver" => data_saver = true,
                "dns-cacher" => cacher_ip = args[i + 1].clone(),
                "dns-provider" => dns_ip = args[i + 1].clone(),
                "resolve" => dest_fqdn = args[i + 1].clone(),
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {}", arg),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'c' => {
                        dns_ip = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'd' => {
                        dns_ip = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'r' => {
                        dest_fqdn = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    's' => data_saver = true,
                    'v' => verbose_level += 1,
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
    let dest_fqdn_clone = dest_fqdn.clone();
    let mut cache_handle =
        task::spawn(async move { cache_task(&cacher_ip, &dest_fqdn_clone).await }).fuse();
    let dest_fqdn_clone = dest_fqdn.clone();
    let mut dns_handle =
        task::spawn(async move { dns_task(&dns_ip, &dest_fqdn_clone).await }).fuse();
    let mut comparison = None;
    let data = select! {
        result = cache_handle => {
            let mut return_data = None;
            if result.is_some() {
                info!("Cache handle returned first");
                return_data = get_data(&result.clone().unwrap(), data_saver).await;
                comparison = Some(task::spawn(compare_results(result.unwrap(), dns_handle, true)));
            }
            else {
                warn!("Cache handle returned None! Fallback to DNS handle.");
                let dns_res = dns_handle.await;
                if dns_res.is_some() {
                    return_data = get_data(&dns_res.clone().unwrap(), data_saver).await;
                }
                else {
                    warn!("Unable to resolve {}!", dest_fqdn);
                }
            }
            return_data
        }
        result = dns_handle => {
            let mut return_data = None;
            if result.is_some() {
                info!("DNS handle returned first");
                return_data = get_data(&result.clone().unwrap(), data_saver).await;
                comparison = Some(task::spawn(compare_results(result.unwrap(), cache_handle, false)));
            }
            else {
                warn!("DNS handle returned None! Fallback to cache handle.");
                let cache_res = cache_handle.await;
                if cache_res.is_some() {
                    return_data = get_data(&cache_res.clone().unwrap(), data_saver).await;
                }
                else {
                    warn!("Unable to resolve {}!", dest_fqdn);
                }
            }
            return_data
        }
    };
    if data.is_some() {
        let response = data.unwrap();
        match response.len().cmp(&4) {
            Ordering::Less => {
                error!("Server send an invalid response.");
                return;
            }
            Ordering::Equal => {
                decode_error(&response.try_into().unwrap_or_default());
                return;
            }
            _ => {}
        }
        match u32::from_le_bytes(response[0..4].try_into().unwrap()) {
            200 => {
                // 200: Server OK / Success
                let plaintext = String::from_utf8_lossy(&response[4..]);
                println!("{}", plaintext);
            }
            999 => {
                //this will never happen, just want clippy to shut the fuck up
            }
            _ => {}
        }
    }
    if comparison.is_some() {
        comparison.unwrap().await;
    }
}

async fn dns_task(dns_ip: &str, dest_fqdn: &str) -> Option<String> {
    if dns_ip != String::new() {
        trace!("Attempting to resolve DNS Server {}", dns_ip);
        let Ok(stream) = TcpStream::connect(dns_ip) else {
            warn!("Failed to resolve to DNS Server {}!", dns_ip);
            return None;
        };
        info!("Connected to {}", dns_ip);
        debug!("Attempting to resolve {}", dest_fqdn);
        let dest_ip = dns_resolve(&stream, dest_fqdn, "", dns_ip);
        info!(
            "Resolved {} to {}!",
            dest_fqdn,
            dest_ip.clone().unwrap_or_default()
        );
        if dest_ip != Some(String::new()) {
            return dest_ip;
        }
    }
    None
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

async fn cache_task(cacher_ip: &str, dest_fqdn: &str) -> Option<String> {
    if cacher_ip != String::new() {
        trace!("Contacting DNS Cacher {}", cacher_ip);
        let Ok(stream) = TcpStream::connect(cacher_ip) else {
            warn!("Failed to contact DNS Cacher {}!", cacher_ip);
            return None;
        };
        info!("Connected to {}", cacher_ip);
        debug!("Locating {}", dest_fqdn);
        let dest_ip = cache_resolve(&stream, dest_fqdn, cacher_ip);
        info!(
            "Resolved {} to {}!",
            dest_fqdn,
            dest_ip.clone().unwrap_or_default()
        );
        if dest_ip != Some(String::new()) {
            return dest_ip;
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

async fn get_data(loc: &str, datasave: bool) -> Option<Vec<u8>> {
    let Ok(stream) = TcpStream::connect(loc) else {
        error!("Failed to connect to {}!", loc);
        return None;
    };
    let mut payload = PTCL_VER.0.to_le_bytes().to_vec();
    payload.extend_from_slice(&PTCL_VER.1.to_le_bytes());
    if datasave {
        payload.extend_from_slice("index.md".as_bytes());
        send_data(&payload, &stream);
        return Some(receive_data(&stream));
    }
    None
}

async fn compare_results(
    complete: String,
    future: futures::future::Fuse<task::JoinHandle<Option<String>>>,
    _cache_completed: bool,
) {
    let result = future.await;
    if result.is_some() && result.unwrap() != complete {
        error!("DNS Server and DNS Cacher returned different results!");
    }
}

fn decode_error(response: &[u8; 4]) {
    match u32::from_le_bytes(*response) {
        400 => error!("Bad request."),
        402 => error!("Payload too small."),
        403 => error!("Forbidden action."),
        404 => error!("Resource not found."),
        426 => error!("Upgrade required."),
        427 => error!("Downgrade required."),
        501 => error!("Operation not implemented."),
        _ => error!("Communication fault."),
    }
}
