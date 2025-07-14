use async_std::path::Path;
use std::{
    cmp::Ordering,
    env,
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
};
use tracing::{Level, debug, error, info, trace, warn};
use utils::{receive_data, send_data, send_error, version_compare};

const DEFAULT_PORT: u16 = 6204;
const SERVER_PTCLS: [&[u8; 5]; 4] = [b"HTML!", b"MRKDN", b"CRAWL", b"RWDTA"];
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
    let mut pwd = String::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "directory" => pwd = args[i + 1].clone(),
                "port" => portstr = args[i + 1].clone(),
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'd' => {
                        pwd = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'p' => {
                        portstr = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'v' => verbose_level += 1,
                    _ => panic!("Pre-init failure; unknown short-name argument: {arg}"),
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
    if pwd.is_empty() {
        pwd = match env::current_dir() {
            Ok(pwd) => pwd.display().to_string(),
            Err(e) => {
                error!("Failed to get working directory directory: {}", e);
                return;
            }
        }
    } else {
        let directory = Path::new(&pwd);
        if !directory.is_dir().await {
            error!("Directory does not exist: {}", &pwd);
            return;
        }
    }
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
                let directory = pwd.clone();
                async_std::task::spawn(async move {
                    handle_connection(stream, &directory).await;
                });
            }
        }
    }
}

async fn handle_connection(stream: TcpStream, directory: &str) {
    let version: Vec<u32> = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|f| f.parse::<u32>().unwrap())
        .collect();
    let peer = match stream.peer_addr() {
        Ok(peer) => peer,
        Err(e) => {
            warn!("Some fuckn' loser decided to not have an IP address: {}", e);
            send_error(&stream, 400);
            return;
        }
    };
    let data = receive_data(&stream);
    if data.len() < 14 {
        warn!("Payload from {}:{} was too short.", peer.ip(), peer.port());
        send_error(&stream, 402);
        return;
    }
    let client_maj = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0, 0, 0, 0]));
    let client_min = u32::from_le_bytes(data[4..8].try_into().unwrap_or([0, 0, 0, 0]));
    let client_tiny = u32::from_le_bytes(data[8..12].try_into().unwrap_or([0, 0, 0, 0]));
    match version_compare((client_maj, client_min, client_tiny), peer, version) {
        Ordering::Greater => send_error(&stream, 427),
        Ordering::Less => send_error(&stream, 426),
        _ => (),
    }
    let mut data = &data[12..];
    if data.is_empty() {
        warn!("Payload from {}:{} was too short.", peer.ip(), peer.port());
        send_error(&stream, 402);
        return;
    }
    let mut client_protocols = vec![[0u8; 5]];
    loop {
        if data.len() < 5 {
            warn!("Payload from {}:{} was too short.", peer.ip(), peer.port());
            send_error(&stream, 402);
            return;
        }
        client_protocols.push(data[0..5].try_into().unwrap_or([0u8; 5]));
        data = &data[5..];
        if data[0] == b'/' {
            data = data.get(1..).unwrap_or_default();
            break;
        }
    }
    let mut using_protocol = None;
    for protocol in client_protocols {
        if SERVER_PTCLS.contains(&&protocol) {
            using_protocol = Some(protocol);
            break;
        }
    }
    if using_protocol.is_none() {
        send_error(&stream, 422);
        return;
    }
    let using_protocol = using_protocol.unwrap();
    let location = String::from_utf8_lossy(&data[0..]);
    let protocol = using_protocol;
    if &protocol == SERVER_PTCLS[0] {
        html_content(&stream, &location, directory).await;
    } else if &protocol == SERVER_PTCLS[1] {
        markdown_content(&stream, &location, directory).await;
    } else if &protocol == SERVER_PTCLS[2] {
        crawl_content(&stream, &location, directory).await;
    } else if &protocol == SERVER_PTCLS[3] {
        raw_data_content(&stream, &location, directory).await;
    } else {
        error!("This path is unreachable!");
    };
}

async fn html_content(stream: &TcpStream, _destination: &str, _directory: &str) {
    send_data(&0u32.to_le_bytes(), stream);
}
async fn markdown_content(stream: &TcpStream, destination: &str, directory: &str) {
    let path = Path::new(destination);
    let path = Path::new(&directory).join(path);
    if !pathcheck(&path, Path::new(directory)).await {
        send_error(stream, 403);
        return;
    }
    if !path.is_dir().await {
        send_error(stream, 404);
        return;
    }
    let content = path.join("index.md");
    if !content.is_file().await {
        send_error(stream, 404);
        return;
    }
    debug!(
        "Client requested markdown content for path: {}",
        &destination
    );
    let mut payload = 200u32.to_le_bytes().to_vec();
    payload.extend_from_slice(SERVER_PTCLS[1]);
    let filedump = File::open(&content);
    match filedump {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            match file.read_to_end(&mut buffer) {
                Ok(_) => {
                    payload.extend_from_slice(&buffer);
                }
                Err(e) => {
                    warn!("Failed to read file: {}", e);
                    send_error(stream, 404);
                    return;
                }
            }
        }
        Err(e) => {
            warn!("Failed to open file: {}", e);
            send_error(stream, 404);
            return;
        }
    }
    send_data(&payload, stream);
}
async fn crawl_content(stream: &TcpStream, _destination: &str, _directory: &str) {
    send_data(&0u32.to_le_bytes(), stream);
}
async fn raw_data_content(stream: &TcpStream, content: &str, directory: &str) {
    let path = Path::new(content);
    let path = Path::new(&directory).join(path);
    if !pathcheck(&path, Path::new(directory)).await {
        send_error(stream, 403);
        return;
    }
    if !path.is_file().await {
        send_error(stream, 404);
        return;
    }
    debug!("Client requested file dump for file: {}", &content);
    let mut payload = 200u32.to_le_bytes().to_vec();
    payload.extend_from_slice(SERVER_PTCLS[1]);
    let filedump = File::open(&path);
    match filedump {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            match file.read_to_end(&mut buffer) {
                Ok(_) => {
                    payload.extend_from_slice(&buffer);
                }
                Err(e) => {
                    warn!("Failed to read file: {}", e);
                    send_error(stream, 404);
                    return;
                }
            }
        }
        Err(e) => {
            warn!("Failed to open file: {}", e);
            send_error(stream, 404);
            return;
        }
    }
    send_data(&payload, stream);
}

async fn pathcheck(newpath: &Path, origpath: &Path) -> bool {
    let newpath = newpath
        .canonicalize()
        .await
        .unwrap_or_else(|_| newpath.to_path_buf());
    let origpath = origpath
        .canonicalize()
        .await
        .unwrap_or_else(|_| origpath.to_path_buf());
    newpath.starts_with(&origpath)
}
