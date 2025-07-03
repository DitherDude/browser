use sqlx::mysql::MySqlPool;
use std::{
    cmp::Ordering,
    env,
    net::{TcpListener, TcpStream},
};
use tracing::{Level, debug, error, info, trace, warn};
use utils::{receive_data, send_data};

const DEFAULT_PORT: u16 = 6202;
const PTCL_VER: (u32, u32, u32) = (0, 0, 0);

#[async_std::main]
async fn main() {
    let mut verbose_level = 0u8;
    let args: Vec<String> = env::args().collect();
    let mut portstr = DEFAULT_PORT.to_string();
    let mut sql_url = String::from("mysql://root:password@localhost:3306/dns");
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                "port" => portstr = args[i + 1].clone(),
                "sql-url" => sql_url = args[i + 1].clone(),
                _ => panic!("Pre-init failure; unknown long-name argument: {}", arg),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    'p' => {
                        portstr = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    's' => {
                        sql_url = args[argindex + 1].clone();
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
    trace!("Attempting to connect to database...");
    match MySqlPool::connect(&sql_url).await {
        Ok(pool) => {
            debug!("Database connection successful!");
            check_database(&pool).await;
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return;
        }
    }
    let listener = match TcpListener::bind("127.0.0.1:".to_owned() + &port.to_string()) {
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
                let sql_url = sql_url.clone();
                async_std::task::spawn(async move {
                    handle_connection(stream, &sql_url).await;
                });
            }
        }
    }
}

async fn handle_connection(stream: TcpStream, sql_url: &str) {
    let peer = match stream.peer_addr() {
        Ok(peer) => peer,
        Err(e) => {
            warn!("Some fuckn' loser decided to not have an IP address: {}", e);
            send_error(&stream, 400);
            return;
        }
    };
    let data = receive_data(&stream);
    let request = String::from_utf8_lossy(&data[9..]);
    info!(
        "Connection from {}:{} requesting {}.",
        peer.ip(),
        peer.port(),
        request
    );
    if data.len() < 10 {
        warn!(
            "Connection from {}:{} was too short.",
            peer.ip(),
            peer.port()
        );
        send_error(&stream, 402);
        return;
    }
    let client_maj = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let client_min = u32::from_le_bytes(data[4..8].try_into().unwrap());
    match version_compare((client_maj, client_min), peer) {
        Ordering::Greater => send_error(&stream, 427),
        Ordering::Less => send_error(&stream, 426),
        _ => (),
    }
    let payload = resolve(&request, sql_url, data[9] == 0).await;
    send_data(&payload, &stream);
    stream
        .shutdown(std::net::Shutdown::Both)
        .unwrap_or_default();
}

async fn resolve(destination: &str, sql_url: &str, is_last_block: bool) -> Vec<u8> {
    trace!("Resolving {}.", destination);
    trace!("Connecting to database...");
    debug!("Database connection URL: {}", sql_url);
    let pool = match MySqlPool::connect(sql_url).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return 421u32.to_le_bytes().to_vec();
        }
    };
    if let Ok(record) = sqlx::query!(
        r#"
        SELECT dns_ip, dns_port
        FROM dns_records
        WHERE name = ?
        "#,
        ".".to_owned()
    )
    .fetch_one(&pool)
    .await
    {
        let dns_ip = record.dns_ip;
        let dns_port = record.dns_port;
        if dns_ip.is_some() && dns_port.is_some() {
            let return_addr = format!("{}:{}", dns_ip.unwrap(), dns_port.unwrap());
            debug!(
                "This DNS server {} has moved to {}!",
                destination, return_addr
            );
            let mut payload = 301u32.to_le_bytes().to_vec();
            payload.extend_from_slice(return_addr.as_bytes());
            return payload;
        }
    }
    match sqlx::query!(
        r#"
        SELECT domain_ip, domain_port, dns_ip, dns_port
        FROM dns_records
        WHERE name = ?
        "#,
        destination
    )
    .fetch_one(&pool)
    .await
    {
        Ok(record) => {
            let domain_ip = record.domain_ip;
            let domain_port = record.domain_port;
            let dns_ip = record.dns_ip;
            let dns_port = record.dns_port;
            if is_last_block {
                if domain_ip.is_some() && domain_port.is_some() {
                    let return_addr = format!("{}:{}", domain_ip.unwrap(), domain_port.unwrap());
                    trace!("Resolved {} to {}.", destination, return_addr);
                    let mut payload = 200u32.to_le_bytes().to_vec();
                    payload.extend_from_slice(return_addr.as_bytes());
                    return payload;
                } else if dns_ip.is_some() && dns_port.is_some() {
                    let return_addr = format!("{}:{}", dns_ip.unwrap(), dns_port.unwrap());
                    trace!("Resolved {} to {}.", destination, return_addr);
                    let mut payload = 302u32.to_le_bytes().to_vec();
                    payload.extend_from_slice(return_addr.as_bytes());
                }
                warn!("Failed to resolve {}.", destination);
                return 410u32.to_le_bytes().to_vec();
            } else if dns_ip.is_some() && dns_port.is_some() {
                let return_addr = format!("{}:{}", dns_ip.unwrap(), dns_port.unwrap());
                trace!("Resolved {} to DNS {}.", destination, return_addr);
                let mut payload = 302u32.to_le_bytes().to_vec();
                payload.extend_from_slice(return_addr.as_bytes());
                return payload;
            } else if domain_ip.is_some() && domain_port.is_some() {
                let return_addr = format!("{}:{}", domain_ip.unwrap(), domain_port.unwrap());
                trace!("Resolved {} to {}.", destination, return_addr);
                let mut payload = 200u32.to_le_bytes().to_vec();
                payload.extend_from_slice(return_addr.as_bytes());
                return payload;
            }
            warn!("Failed to resolve {}.", destination);
            return resolve_wildcard(&pool).await;
        }
        Err(e) => {
            warn!("Failed to fetch record for {}: {}", destination, e);
            return resolve_wildcard(&pool).await;
        }
    }
}

fn send_error(stream: &TcpStream, err: i32) {
    send_data(&err.to_le_bytes(), stream);
    stream
        .shutdown(std::net::Shutdown::Both)
        .unwrap_or_default();
}

fn version_compare(client: (u32, u32), peer: std::net::SocketAddr) -> Ordering {
    if client.0 > PTCL_VER.0 {
        warn!(
            "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
            peer.ip(),
            peer.port(),
            client.0,
            client.1,
            PTCL_VER.0,
            PTCL_VER.1
        );
        return Ordering::Greater;
    } else if client.0 < PTCL_VER.0 || (client.0 == PTCL_VER.0 && client.1 < PTCL_VER.1) {
        warn!(
            "Connection from {}:{} used an incompatible protocol: {}.{}, expected {}.{}",
            peer.ip(),
            peer.port(),
            client.0,
            client.1,
            PTCL_VER.0,
            PTCL_VER.1
        );
        return Ordering::Less;
    }
    Ordering::Equal
}

async fn resolve_wildcard(pool: &MySqlPool) -> Vec<u8> {
    debug!("Fetching wildcard record...");
    match sqlx::query!(
        r#"
        SELECT domain_ip, domain_port
        FROM dns_records
        WHERE name = ?
        "#,
        "."
    )
    .fetch_one(pool)
    .await
    {
        Ok(record) => {
            let domain_ip = record.domain_ip;
            let domain_port = record.domain_port;
            if domain_ip.is_some() && domain_port.is_some() {
                let return_addr = format!("{}:{}", domain_ip.unwrap(), domain_port.unwrap());
                let mut payload = 203u32.to_le_bytes().to_vec();
                payload.extend_from_slice(return_addr.as_bytes());
                return payload;
            }
            warn!("Wildcard error exists, but missing ip record.");
        }
        Err(e) => {
            warn!("Failed to fetch wildcard record: {}", e);
        }
    }
    421u32.to_le_bytes().to_vec()
}

async fn check_database(pool: &MySqlPool) {
    trace!("Checking database integrity...");
    let sql = r#"
    CREATE TABLE IF NOT EXISTS dns_records (
        id INT AUTO_INCREMENT PRIMARY KEY,
        name VARCHAR(255) UNIQUE NOT NULL,
        domain_ip VARCHAR(63) NULL,
        domain_port SMALLINT UNSIGNED NULL CHECK (domain_port BETWEEN 0 AND 25565),
        dns_ip VARCHAR(63) NULL,
        dns_port SMALLINT UNSIGNED NULL CHECK (dns_port BETWEEN 0 AND 25565)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    match sqlx::query(sql).execute(pool).await {
        Ok(_) => {
            trace!("Database integrity check passed.");
        }
        Err(e) => {
            panic!("Failed to create database: {}", e);
        }
    };
}
