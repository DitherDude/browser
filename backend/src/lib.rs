use async_std::task;
use futures::{FutureExt, select};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::{
    cmp::Ordering,
    env,
    net::TcpStream,
    path::{self, PathBuf},
};
use tracing::{debug, error, info, trace, warn};
use utils::{fqdn_to_upe, get_config_dir, receive_data, send_data, sql_cols, status};

const DNS_IP: &str = "0.0.0.0:6202";
const CACHER_IP: &str = "0.0.0.0:6203";

pub async fn resolve(
    dest_addr: &str,
    integrity_check: Option<bool>,
    dns_ip: Option<&str>,
    cacher_ip: Option<&str>,
) -> (String, u32) {
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
    let mut result = (String::new(), status::HOST_UNREACHABLE);
    let dest_addr_clone = dest_addr.to_owned();
    let mut cache_handle =
        task::spawn(async move { cache_task(&cacher_ip, &dest_addr_clone).await }).fuse();
    let dest_addr_clone = dest_addr.to_owned();
    let mut dns_handle =
        task::spawn(async move { dns_task(&dns_ip, &dest_addr_clone).await }).fuse();
    let mut comparison = None;
    let data = select! {
        result = cache_handle => {
            let mut return_data = (None, result.1);
            match result.0 {
                Some(address) => {
                    info!("Cache handle returned first");
                    return_data.0 = Some(address.clone());
                    if integrity_check {comparison = Some(task::spawn(compare_results(address, dns_handle)));}
                }
                None => {
                    warn!("Cache handle returned None! Fallback to DNS handle.");
                    let dns_res = dns_handle.await;
                    if dns_res.0.is_some() {
                        return_data = dns_res;
                    }
                    else {
                        warn!("Unable to resolve {}!", dest_addr);
                    }
                }
            }
            return_data
        }
        result = dns_handle => {
            let mut return_data = (None, result.1);
            match result.0 {
                Some(address) => {
                    info!("DNS handle returned first");
                    return_data.0 = Some(address.clone());
                    if integrity_check {comparison = Some(task::spawn(compare_results(address, cache_handle)));}
                }
                None => {
                    warn!("DNS handle returned None! Fallback to cache handle.");
                    let cache_res = cache_handle.await;
                    if cache_res.0.is_some() {
                        return_data = cache_res;
                    }
                    else {
                        warn!("Unable to resolve {}!", dest_addr);
                    }
                }
            }
            return_data
        }
    };
    match data.0 {
        Some(response) => {
            result = (response, data.1);
        }
        None => error!("Unable to resolve {}.", dest_addr),
    }
    if let Some(comparison) = comparison
        && integrity_check
    {
        comparison.await;
    }
    result
}

pub async fn dns_task(dns_ip: &str, dest_addr: &str) -> (Option<String>, u32) {
    let (dest_url, _, _) = fqdn_to_upe(dest_addr);
    if dns_ip != String::new() {
        trace!("Attempting to resolve DNS Server {}", dns_ip);
        let Ok(stream) = TcpStream::connect(dns_ip) else {
            warn!("Failed to resolve to DNS Server {}!", dns_ip);
            return (None, status::HOST_UNREACHABLE);
        };
        info!("Connected to {}", dns_ip);
        debug!("Attempting to resolve {}", dest_url);
        let dest = dns_resolve(&stream, &dest_url, "", dns_ip, &["".to_string()]);
        info!(
            "Resolved {} to {}!",
            dest_url,
            dest.clone().0.unwrap_or_default()
        );
        if let Some(dest_ip) = dest.0 {
            if dest_ip == String::new() {
                return (None, dest.1);
            } else {
                return (Some(dest_ip), dest.1);
            };
        }
    }
    (None, status::HOST_UNREACHABLE)
}

fn dns_resolve(
    stream: &TcpStream,
    destination: &str,
    prev: &str,
    dns_ip: &str,
    routes: &[String],
) -> (Option<String>, u32) {
    let block = destination.split('.').next_back().unwrap_or_default();
    let next_prev = ".".to_owned() + block + prev;
    let is_last_block = block == destination;
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
    let mut payload = program_version[0].to_le_bytes().to_vec();
    payload.extend_from_slice(&program_version[1].to_le_bytes());
    payload.extend_from_slice(&program_version[2].to_le_bytes());
    payload.extend_from_slice(if is_last_block { &[0u8] } else { &[1u8] });
    payload.extend_from_slice(block.as_bytes());
    send_data(&payload, stream);
    let response = receive_data(stream);
    match response.len().cmp(&4) {
        Ordering::Less => {
            error!("Server send an invalid response.");
            return (None, status::BAD_RESPONSE);
        }
        Ordering::Equal => {
            return (None, u32::from_le_bytes(response.try_into().unwrap()));
        }
        _ => {}
    }
    let statuscode = u32::from_le_bytes(response[0..4].try_into().unwrap());
    match statuscode {
        status::SUCCESS => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("DNS resolved to destination {} early.", fqdn);
            }
            return (Some(fqdn.into_owned()), statuscode);
        }
        status::NON_AUTHORITATIVE => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!(
                "DNS fallback configured to correct FQN {} where doesn't exist.",
                fqdn
            );
            return (Some(fqdn.into_owned()), statuscode);
        }
        status::PERMANENT_REDIRECT => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!("DNS Server {} has moved to {}!", dns_ip, fqdn);
            trace!("Attempting to connect to new DNS Server {}", fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve new DNS Server {}!", fqdn);
                return (None, statuscode);
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
                    return (None, status::LOOP_DETECTED);
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
                return (None, statuscode);
            };
            debug!("Attempting to resolve {}", newdestination);
            return dns_resolve(&newstream, &newdestination, &next_prev, &fqdn, &routes);
        }
        status::GONE => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            if !is_last_block {
                warn!("Reached end of DNS chain early! Rectifying FQN as {}", fqdn);
            }
            return (Some(fqdn.into_owned()), statuscode);
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
    (None, status::HOST_UNREACHABLE)
}

async fn cache_task(cacher_ip: &str, dest_addr: &str) -> (Option<String>, u32) {
    let (dest_url, _, _) = fqdn_to_upe(dest_addr);
    if cacher_ip != String::new() {
        trace!("Contacting DNS Cacher {}", cacher_ip);
        let Ok(stream) = TcpStream::connect(cacher_ip) else {
            warn!("Failed to contact DNS Cacher {}!", cacher_ip);
            return (None, status::HOST_UNREACHABLE);
        };
        info!("Connected to {}", cacher_ip);
        debug!("Locating {}", dest_url);
        let dest = cache_resolve(&stream, &dest_url, cacher_ip);
        info!(
            "Resolved {} to {}!",
            dest_url,
            dest.clone().0.unwrap_or_default()
        );
        if let Some(dest_ip) = dest.0 {
            if dest_ip == String::new() {
                return (None, dest.1);
            } else {
                return (Some(dest_ip), dest.1);
            };
        }
    }
    (None, status::HOST_UNREACHABLE)
}

fn cache_resolve(stream: &TcpStream, destination: &str, dns_ip: &str) -> (Option<String>, u32) {
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
    let mut payload = program_version[0].to_le_bytes().to_vec();
    payload.extend_from_slice(&program_version[1].to_le_bytes());
    payload.extend_from_slice(&program_version[2].to_le_bytes());
    payload.extend_from_slice(destination.as_bytes());
    send_data(&payload, stream);
    let response = receive_data(stream);
    match response.len().cmp(&4) {
        Ordering::Less => {
            error!("Server send an invalid response.");
            return (None, status::BAD_RESPONSE);
        }
        Ordering::Equal => {
            return (None, u32::from_le_bytes(response.try_into().unwrap()));
        }
        _ => {}
    }
    let statuscode = u32::from_le_bytes(response[0..4].try_into().unwrap());
    match statuscode {
        status::SUCCESS => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            return (Some(fqdn.into_owned()), statuscode);
        }
        status::PERMANENT_REDIRECT => {
            let fqdn = String::from_utf8_lossy(&response[4..]);
            warn!("DNS Cacher {} has moved to {}!", dns_ip, fqdn);
            trace!("Attempting to connect to new DNS Cacher {}", fqdn);
            let Ok(newstream) = TcpStream::connect(fqdn.to_string()) else {
                error!("Failed to resolve new DNS Cacher {}!", fqdn);
                return (None, statuscode);
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
    (None, status::HOST_UNREACHABLE)
}

async fn compare_results(
    complete: String,
    future: futures::future::Fuse<task::JoinHandle<(Option<String>, u32)>>,
) {
    let result = future.await.0;
    if result.is_some() && result.unwrap() != complete {
        error!("DNS Server and DNS Cacher returned different results!");
        //report to DNS cacher that its information is outdated
    }
}

pub async fn parse_stack(elements: &str, stack: &str, applet: &str) -> Option<gtk4::Box> {
    let config_dir = match get_config_dir(applet) {
        Some(dir) => dir,
        None => return None,
    };
    let dbpath = config_dir.join(path::Path::new("stacks.db"));
    let pool = match SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(dbpath)
            .create_if_missing(false),
    )
    .await
    {
        Ok(pool) => pool,
        Err(e) => {
            error!("Database error: {}", e);
            return None;
        }
    };
    if let Ok(Some(record)) =
        sqlx::query_as::<_, sql_cols::StacksRecord>("SELECT * FROM stacks WHERE stack = ?")
            .bind(stack)
            .fetch_optional(&pool)
            .await
    {
        let libloc = path::Path::new(&record.library);
        pub fn parser(libloc: &path::Path, elements: &str) -> Option<gtk4::Box> {
            unsafe {
                let lib = match libloading::Library::new(libloc) {
                    Ok(lib) => lib,
                    Err(_) => return None,
                };
                let func: libloading::Symbol<fn(elements: String) -> Option<gtk4::Box>> =
                    match lib.get("get_elements".as_bytes()) {
                        Ok(data) => data,
                        Err(_) => return None,
                    };
                func(elements.to_owned())
            }
        }
        return parser(libloc, elements);
    }
    None
}

pub fn get_stack_info(location: &PathBuf) -> Option<String> {
    unsafe {
        let lib = match libloading::Library::new(location) {
            Ok(lib) => lib,
            Err(_) => return None,
        };
        let func: libloading::Symbol<fn() -> Option<String>> = match lib.get("stacks".as_bytes()) {
            Ok(data) => data,
            Err(_) => return None,
        };
        func()
    }
}
