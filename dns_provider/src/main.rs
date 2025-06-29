use sqlx::mysql::MySqlPool;
use std::{
    env,
    net::{TcpListener, TcpStream},
};
use tracing::{Level, debug, error, info, trace, warn};

#[async_std::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut verbose_level = 0u8;
    let mut port = 840u16;
    let mut sql_url = String::from("mysql://root@localhost:3306/dns_provider");
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                "port" => {
                    port = match args[i + 1].parse() {
                        Ok(p) => p,
                        Err(e) => {
                            warn!("Failed to parse port: {}. Defaulting to 840", e);
                            840
                        }
                    }
                }
                "sql-url" => sql_url = args[i + 1].clone(),
                _ => panic!("Pre-init failure; unknown long-name argument: {}", arg),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    'p' => {
                        port = match args[argindex + 1].parse() {
                            Ok(p) => p,
                            Err(e) => {
                                warn!("Failed to parse port: {}. Defaulting to 840", e);
                                840
                            }
                        };
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
    trace!("Attempting to connect to database...");
    match MySqlPool::connect(&sql_url).await {
        Ok(_pool) => {
            debug!("Database connection successful!");
            // Todo
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return;
        }
    }
    let listener = match TcpListener::bind("127.0.0.1:".to_owned() + &port.to_string()) {
        Ok(listener) => listener,
        Err(e) => {
            error!("Port is busy! {}", e);
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
                async_std::task::spawn(async move {
                    handle_connection(stream).await;
                });
            }
        }
    }
}

async fn handle_connection(mut _stream: TcpStream) {
    //Todo
}
