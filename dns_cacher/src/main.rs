use sqlx::mysql::MySqlPool;
use std::{
    cmp::Ordering,
    env,
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info, trace, warn};
use utils::{receive_data, send_data, send_error, trace_subscription, version_compare};

const DEFAULT_PORT: u16 = 6203;

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
    let mut sql_url = String::from("mysql://root:password@localhost:3306/dns");
    let mut overwrite = false;
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "overwrite" => overwrite = true,
                "port" => portstr = args[i + 1].clone(),
                "sql-url" => sql_url = args[i + 1].clone(),
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'o' => overwrite = true,
                    'p' => {
                        portstr = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    's' => {
                        sql_url = args[argindex + 1].clone();
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
    trace!("Attempting to connect to database...");
    match MySqlPool::connect(&sql_url).await {
        Ok(pool) => {
            debug!("Database connection successful!");
            check_database(&pool, overwrite).await;
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
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
                let sql_url = sql_url.clone();
                async_std::task::spawn(async move {
                    handle_connection(stream, &sql_url).await;
                });
            }
        }
    }
}

async fn handle_connection(stream: TcpStream, sql_url: &str) {
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
    if data.len() < 13 {
        warn!("Payload from {}:{} was too short.", peer.ip(), peer.port());
        send_error(&stream, 402);
        return;
    }
    let request = String::from_utf8_lossy(&data[12..]);
    info!(
        "Connection from {}:{} requesting {}.",
        peer.ip(),
        peer.port(),
        request
    );
    let client_maj = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0, 0, 0, 0]));
    let client_min = u32::from_le_bytes(data[4..8].try_into().unwrap_or([0, 0, 0, 0]));
    let client_tiny = u32::from_le_bytes(data[8..12].try_into().unwrap_or([0, 0, 0, 0]));
    match version_compare((client_maj, client_min, client_tiny), peer, version) {
        Ordering::Greater => send_error(&stream, 427),
        Ordering::Less => send_error(&stream, 426),
        _ => (),
    }
    let payload = resolve(&request, sql_url).await;
    send_data(&payload, &stream);
    stream
        .shutdown(std::net::Shutdown::Both)
        .unwrap_or_default();
}

async fn resolve(destination: &str, sql_url: &str) -> Vec<u8> {
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
        SELECT domain_ip, domain_port
        FROM dns_cache
        WHERE name = ?
        "#,
        ".".to_owned()
    )
    .fetch_one(&pool)
    .await
    {
        let domain_ip = record.domain_ip;
        let domain_port = record.domain_port;
        if domain_ip.is_some() && domain_port.is_some() {
            let return_addr = format!("{}:{}", domain_ip.unwrap(), domain_port.unwrap());
            debug!(
                "This DNS cache server {} has moved to {}!",
                destination, return_addr
            );
            let mut payload = 301u32.to_le_bytes().to_vec();
            payload.extend_from_slice(return_addr.as_bytes());
            return payload;
        }
    }
    match sqlx::query!(
        r#"
        SELECT domain_ip, domain_port
        FROM dns_cache
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
            if domain_ip.is_some() && domain_port.is_some() {
                let return_addr = format!("{}:{}", domain_ip.unwrap(), domain_port.unwrap());
                trace!("Resolved {} to {}.", destination, return_addr);
                let mut payload = 200u32.to_le_bytes().to_vec();
                payload.extend_from_slice(return_addr.as_bytes());
                return payload;
            }
            warn!("Failed to resolve {}.", destination);
            421u32.to_le_bytes().to_vec()
        }
        Err(e) => {
            warn!("Failed to fetch record for {}: {}", destination, e);
            421u32.to_le_bytes().to_vec()
        }
    }
}

async fn check_database(pool: &MySqlPool, overwrite: bool) {
    trace!("Checking database schema integrity...");
    match sqlx::query!(
        r#"
        SELECT
            COUNT(*) as count
        FROM
            INFORMATION_SCHEMA.COLUMNS
        WHERE
            TABLE_NAME = 'dns_cache'
        AND (
            (COLUMN_NAME = 'id' AND DATA_TYPE = 'int' AND IS_NULLABLE = 'NO')
            OR (COLUMN_NAME = 'name' AND DATA_TYPE = 'varchar' AND CHARACTER_MAXIMUM_LENGTH = 255 AND IS_NULLABLE = 'NO')
            OR (COLUMN_NAME = 'domain_ip' AND DATA_TYPE = 'varchar' AND CHARACTER_MAXIMUM_LENGTH = 63 AND IS_NULLABLE = 'YES')
            OR (COLUMN_NAME = 'domain_port' AND DATA_TYPE = 'smallint' AND IS_NULLABLE = 'YES')
        );
        "#
    ).fetch_optional(pool).await {
        Ok(Some(e)) => {
            if e.count == 4 {
                trace!("Database schema integrity check passed.");
            } else if overwrite {
                warn!("Database schema mismatch. Will overwite.");
                overwrite_database(pool).await;
                trace!("Database schema overwritten.");
            }
            else {
                error!("Invalid database! Run with --overwrite to overwrite existing database.");
                std::process::exit(1);
            }
        },
        _ => {
            if overwrite {
                warn!("Failure to locate database. Will overwite.");
                overwrite_database(pool).await;
                trace!("Database overwritten.");
            }
            else {
                error!("Failed to locate databse! Run with --overwrite to generate a new database.");
                std::process::exit(1);
            }
        }
    };
}

async fn overwrite_database(pool: &MySqlPool) {
    match sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS dns_cache (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) UNIQUE NOT NULL,
            domain_ip VARCHAR(63) NULL,
            domain_port SMALLINT UNSIGNED NULL CHECK (domain_port BETWEEN 0 AND 25565)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#
    )
    .execute(pool)
    .await
    {
        Ok(_) => {
            info!("Database successfully created.");
        }
        Err(e) => {
            error!("Failed to create database: {}", e);
            std::process::exit(1);
        }
    };
}
