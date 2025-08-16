use std::{
    cmp::Ordering,
    collections::HashMap,
    env,
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tracing::{error, info, trace, warn};
use utils::{receive_data, send_data, send_error, status, trace_subscription, version_compare};

const DEFAULT_PORT: u16 = 6204;
#[async_std::main]
async fn main() {
    let program_version: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|f| match f.parse::<u32>() {
            Ok(version) => version,
            Err(e) => {
                panic!("Failed to parse version: {e}");
            }
        })
        .collect();
    assert!(program_version.len() > 1);
    let mut verbose_level = 0u8;
    let args: Vec<String> = env::args().collect();
    let mut portstr = DEFAULT_PORT.to_string();
    let mut pwd_str = String::new();
    let mut stacksloc_str = String::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "directory" => pwd_str = args[i + 1].clone(),
                "port" => portstr = args[i + 1].clone(),
                "stacks" => stacksloc_str = args[i + 1].clone(),
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'd' => {
                        pwd_str = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'p' => {
                        portstr = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    's' => {
                        stacksloc_str = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'v' => verbose_level += 1,
                    _ => panic!("Pre-init failure; unknown short-name argument: {arg}"),
                }
            }
        }
    }
    trace_subscription(verbose_level);
    let port = match portstr.parse() {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "Failed to parse port: {}. Defaulting to {}",
                e, DEFAULT_PORT
            );
            DEFAULT_PORT
        }
    };
    let pwd = if pwd_str.is_empty() {
        match env::current_dir() {
            Ok(pwd) => pwd,
            Err(e) => {
                error!("Failed to get working directory directory: {}", e);
                return;
            }
        }
    } else {
        let directory = Path::new(&pwd_str);
        if !directory.is_dir() {
            error!("Directory does not exist: {}", &pwd_str);
            return;
        }
        directory.to_path_buf()
    };
    let stacksloc = if stacksloc_str.is_empty() {
        let mut dir = pwd.clone();
        dir.push("stacks.txt");
        if dir.is_file() {
            dir
        } else {
            error!("Cannot find stacks file: {}", &stacksloc_str);
            return;
        }
    } else {
        let file = Path::new(&stacksloc_str);
        if !file.is_file() {
            error!("Cannot find specified stacks file: {}", &stacksloc_str);
            return;
        }
        file.to_path_buf()
    };
    let mut stacksfile = match File::open(&stacksloc) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open stacks file: {}", e);
            return;
        }
    };
    let mut stacks = HashMap::new();
    let mut lines = String::new();
    if stacksfile.read_to_string(&mut lines).is_ok() {
        for line in lines.lines() {
            let (stack, loc) = line.split_at(5);
            stacks.insert(stack.to_string(), loc.to_string());
        }
    } else {
        error!("Failed to read stacks file: {}", &stacksloc_str);
        return;
    }
    let stacks = Arc::new(stacks);
    let listener = match TcpListener::bind("0.0.0.0:".to_owned() + &port.to_string()) {
        Ok(listener) => listener,
        Err(e) => {
            error!("Port is unavailable: {}", e);
            return;
        }
    };
    info!("Listening on port {}. Server setup OK!", port);
    for stream in listener.incoming() {
        match stream {
            Err(e) => {
                warn!("Failed to accept connection: {}", e);
            }
            Ok(stream) => {
                trace!(
                    "New connection from {}:{}",
                    stream.peer_addr().unwrap().ip(),
                    stream.peer_addr().unwrap().port(),
                );
                let dir_clone = pwd.clone();
                let stacks_ptr = Arc::clone(&stacks);
                async_std::task::spawn(async move {
                    handle_connection(stream, &dir_clone, &stacks_ptr).await;
                });
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    directory: &Path,
    stacks: &Arc<HashMap<String, String>>,
) {
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
    let peer = match stream.peer_addr() {
        Ok(peer) => peer,
        Err(e) => {
            warn!("Some fuckn' loser decided to not have an IP address: {}", e);
            send_error(&stream, status::BAD_REQUEST);
            return;
        }
    };
    let data = receive_data(&stream);
    if data.len() < 14 {
        warn!("Payload from {}:{} was too short.", peer.ip(), peer.port());
        send_error(&stream, status::TOO_SMALL);
        return;
    }
    let client_maj = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0, 0, 0, 0]));
    let client_min = u32::from_le_bytes(data[4..8].try_into().unwrap_or([0, 0, 0, 0]));
    let client_tiny = u32::from_le_bytes(data[8..12].try_into().unwrap_or([0, 0, 0, 0]));
    match version_compare((client_maj, client_min, client_tiny), peer, program_version) {
        Ordering::Greater => send_error(&stream, status::DOWNGRADE_REQUIRED),
        Ordering::Less => send_error(&stream, status::UPGRADE_REQUIRED),
        _ => (),
    }
    let mut data = &data[12..];
    if data.is_empty() {
        trace!("Payload from {}:{} was too short.", peer.ip(), peer.port());
        send_error(&stream, status::TOO_SMALL);
        return;
    }
    let mut client_protocols = vec![[0u8; 5]];
    loop {
        if data.len() < 5 {
            trace!("Payload from {}:{} was too short.", peer.ip(), peer.port());
            send_error(&stream, status::TOO_SMALL);
            return;
        }
        client_protocols.push(data[0..5].try_into().unwrap_or([0u8; 5]));
        data = &data[5..];
        if data.is_empty() {
            trace!("Unrecognised request from {}:{}", peer.ip(), peer.port());
            send_error(&stream, status::UNPROCESSABLE);
            return;
        }
        if data[0] == b'/' {
            data = data.get(1..).unwrap_or_default();
            break;
        }
    }
    let mut using_protocol = None;
    let stacks = Arc::clone(stacks);
    for protocol in client_protocols {
        let stack = String::from_utf8_lossy(&protocol).to_string();
        if let Some(protocol) = stacks.get(&stack) {
            using_protocol = Some((stack, protocol.trim().to_string()));
            break;
        }
    }
    let location = String::from_utf8_lossy(&data[0..]);
    match using_protocol {
        None => send_error(&stream, status::UNPROCESSABLE),
        Some(protocol) => get_content(&stream, protocol, directory, &location),
    }
}

fn get_content(
    stream: &TcpStream,
    protocol: (String, String),
    directory: &Path,
    destination: &str,
) {
    let (stack, protocol) = protocol;
    let mut payload = status::SUCCESS.to_le_bytes().to_vec();
    payload.extend_from_slice(stack.as_bytes());
    let file = if protocol.starts_with("/") {
        match get_file(&protocol, directory) {
            Some(content) => content,
            None => {
                send_error(stream, status::SHAT_THE_BED);
                return;
            }
        }
    } else {
        let dest = format!("{destination}/{protocol}");
        match get_file(&dest, directory) {
            Some(content) => content,
            None => {
                send_error(stream, status::NOT_FOUND);
                return;
            }
        }
    };
    let filedump = File::open(&file);
    match filedump {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            match file.read_to_end(&mut buffer) {
                Ok(_) => {
                    payload.extend_from_slice(&buffer);
                }
                Err(e) => {
                    warn!("Failed to read file: {}", e);
                    send_error(stream, status::NOT_FOUND)
                }
            }
        }
        Err(e) => {
            warn!("Failed to open file: {}", e);
            send_error(stream, status::NOT_FOUND)
        }
    }
    send_data(&payload, stream);
}

fn get_file(subpath: &str, directory: &Path) -> Option<PathBuf> {
    let path = match pathcheck(subpath, directory) {
        Some(res) => res,
        _ => {
            return None;
        }
    };
    if !path.is_file() {
        return None;
    }
    Some(path)
}

fn pathcheck(subpath_str: &str, origpath: &Path) -> Option<PathBuf> {
    /*
    The following code is not my own, but adapted slightly to match my use-case.

    Project Title: tower-rs/tower-http
    Snippet Title: build_and_validate_path
    Author(s): carllerche and github:tower-rs:publish
    Date: 03/Jun/2025
    Date Accessed: 10/Aug/2025 01:30AM AEST
    Code version: 0.6.6
    Type: Source Code
    Availability: https://docs.rs/tower-http/latest/src/tower_http/services/fs/serve_dir/mod.rs.html#458-483
    Licence: MIT (docs.rs) / None (github.com)
     */

    let mut finalpath = origpath.to_path_buf();
    let subpath = subpath_str.trim_start_matches('/');
    let subpath = Path::new(subpath);
    for component in subpath.components() {
        match component {
            Component::Normal(comp) => {
                if Path::new(&comp)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                {
                    finalpath.push(comp)
                } else {
                    return None;
                }
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return None;
            }
        }
    }
    Some(finalpath)
}
