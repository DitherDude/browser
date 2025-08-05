use async_std::io;
use gtk::{
    Application, ApplicationWindow,
    glib::{self, clone},
    prelude::*,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::{env, fs, path};
use tracing::{debug, error, trace, warn};
use url_resolver::{dns_task, resolve};
use utils::{get_config_dir, trace_subscription};
const APP_ID: &str = "dither.browser";
const PROJ_NAME: &str = "Browser";

#[async_std::main]
async fn main() -> glib::ExitCode {
    let mut verbose_level = 0u8;
    let args: Vec<String> = env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut _argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    _ => panic!("Pre-init failure; unknown short-name argument: {arg}"),
                }
            }
        }
    }
    trace_subscription(verbose_level);
    let caching = create_cache().await;
    debug!("Caching enabled: {caching}");
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, caching));
    app.run_with_args(&[""])
}

fn build_ui(app: &Application, caching: bool) {
    let widgets = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Browser")
        .default_height(600)
        .default_width(1000)
        .child(&widgets)
        .build();
    let label = gtk::Label::builder()
        .label("Type to start search")
        .vexpand(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["large-title"])
        .build();
    let header_bar = gtk::HeaderBar::new();
    let search_button = gtk::ToggleButton::new();
    search_button.set_icon_name("system-search-symbolic");
    header_bar.pack_end(&search_button);
    let search_bar = gtk::SearchBar::builder()
        .valign(gtk::Align::Start)
        .key_capture_widget(&window)
        .build();
    widgets.append(&search_bar);
    widgets.append(&label);
    window.set_titlebar(Some(&header_bar));
    search_button
        .bind_property("active", &search_bar, "search-mode-enabled")
        .sync_create()
        .bidirectional()
        .build();
    let entry = gtk::SearchEntry::new();
    entry.set_hexpand(true);
    entry.connect_search_started(clone!(
        #[weak]
        search_button,
        move |_| {
            search_button.set_active(true);
        }
    ));
    entry.connect_stop_search(clone!(
        #[weak]
        search_button,
        move |_| {
            search_button.set_active(false);
        }
    ));
    entry.connect_activate(move |entry| {
        let entry_clone = entry.clone();
        let label_weak = gtk::Label::downgrade(&label);
        glib::MainContext::default().spawn_local(async move {
            if let Some(label) = label_weak.upgrade() {
                match try_cache_webpage(&entry_clone, &label, caching).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("FS error: {}", e);
                        label.set_text(
                            &resolve_url(&entry_clone.text())
                                .await
                                .unwrap_or("Website not found.".to_string()),
                        );
                    }
                }
            }
        });
    });
    search_bar.set_child(Some(&entry));
    window.present();
}

async fn try_cache_webpage(
    entry: &gtk::SearchEntry,
    label: &gtk::Label,
    caching: bool,
) -> io::Result<()> {
    label.set_text("Loading...");
    if !caching {
        trace!("Caching disabled. Will resolve directly.");
        label.set_text(
            &resolve_url(&entry.text())
                .await
                .unwrap_or("Website not found.".to_string()),
        );
        return Ok(());
    }
    let config_dir = match get_config_dir(PROJ_NAME) {
        Some(dir) => dir,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::NotSeekable,
                "Could not determine compatable configuration directory.",
            ));
        }
    };
    fs::create_dir_all(&config_dir)?;
    let dbpath = config_dir.join(path::Path::new("cache.db"));
    let pool = match SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(dbpath)
            .create_if_missing(true),
    )
    .await
    {
        Ok(pool) => pool,
        Err(e) => {
            return Err(io::Error::other(format!("Database error: {e}")));
        }
    };
    trace!("Successfully connected to database");
    let text = entry.text();
    let text = text.strip_prefix("web://").unwrap_or(&text);
    let (url, _endpoint) = text.split_once('/').unwrap_or((text, ""));
    let (url, port) = url.split_once(':').unwrap_or((url, ""));
    let port = port.parse::<u16>().ok();
    trace!("URL: {url}, Port: {port:?}, Endpoint: {_endpoint}");
    let mut blocks = url.split('.').collect::<Vec<_>>();
    let mut lookahead = String::new();
    let mut verified_url = None;
    let mut cache_used = false;
    while !blocks.is_empty() {
        trace!("Blocks: {:?}", blocks);
        let sub_url = blocks.join(".");
        warn!("Looking for {}", sub_url);
        if let Ok(Some(record)) =
            sqlx::query_as::<_, Ephemeral>("SELECT * FROM ephemeral WHERE url = ?")
                .bind(&sub_url)
                .fetch_optional(&pool)
                .await
        {
            trace!("Record found for {}!", sub_url);
            cache_used = true;
            let ip = record.ip;
            let dest = if lookahead.is_empty() {
                Some(ip)
            } else {
                let value = dns_task(&ip, &lookahead).await;
                match value {
                    Some(value) => Some(format!("{}:{}", value.0, value.1)),
                    None => None,
                }
            };
            let dest = if dest.is_some() && port.is_some() {
                Some(format!(
                    "{}:{}",
                    dest.unwrap().split_once(':').unwrap_or_default().0,
                    port.unwrap()
                ))
            } else {
                dest
            };
            match dest {
                Some(dest) => {
                    label.set_text(&dest);
                    let verified_url = Some(dest.clone());
                    if let Some(validated_url) = resolve_url(&entry.text()).await {
                        if dest != validated_url {
                            error!("Cache held invalid url!");
                            debug!(
                                "Cache reported {}, but validated to {}",
                                dest, validated_url
                            );
                            match sqlx::query("DELETE FROM ephemeral WHERE url = ?;")
                                .bind(&sub_url)
                                .execute(&pool)
                                .await
                            {
                                Ok(_) => cache_used = false,
                                Err(e) => {
                                    error!("Database error: {}", e);
                                }
                            }
                            verified_url = Some(validated_url);
                        }
                    };
                }
                None => {
                    error!("I honestly forgot what causes this");
                    verified_url = resolve_url(&entry.text()).await;
                }
            }
            break;
        }
        let lastblock = blocks.last().copied().unwrap_or_default().to_owned();
        lookahead = if lookahead.is_empty() {
            lastblock
        } else if lookahead.starts_with(".") {
            lastblock + &lookahead
        } else {
            lastblock + "." + &lookahead
        };
        blocks.pop();
    }
    if blocks.is_empty() {
        warn!("No cache found for {}!", url);
        debug!("resolving {} directly...", url);
        verified_url = resolve_url(&entry.text()).await;
        lookahead = String::new();
    }
    if lookahead.is_empty() && verified_url.is_some() && !cache_used {
        debug!("Caching resolved url");
        match sqlx::query("INSERT INTO ephemeral (url, ip) VALUES (?, ?);")
            .bind(url)
            .bind(verified_url.as_ref().unwrap())
            .execute(&pool)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Database error: {}", e);
            }
        };
    }
    label.set_text(&verified_url.unwrap_or("Website not found.".to_string()));
    Ok(())
}

async fn resolve_url(destination: &str) -> Option<String> {
    let ip = resolve(destination, None, None, None).await;
    if ip.is_empty() { None } else { Some(ip) }
}

async fn create_cache() -> bool {
    let config_dir = match get_config_dir(PROJ_NAME) {
        Some(dir) => dir,
        None => {
            error!("Could not determine compatable configuration directory.");
            return false;
        }
    };
    match fs::create_dir_all(&config_dir) {
        Ok(_) => {}
        Err(e) => {
            error!("FS error: {}", e);
            return false;
        }
    };
    let dbpath = config_dir.join(path::Path::new("cache.db"));
    let pool = match SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(dbpath)
            .create_if_missing(true),
    )
    .await
    {
        Ok(pool) => pool,
        Err(e) => {
            error!("Database error: {}", e);
            return false;
        }
    };
    match sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ephemeral
            (id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT UNIQUE NOT NULL,
            ip TEXT NOT NULL);
        "#,
    )
    .execute(&pool)
    .await
    {
        Ok(_) => {}
        Err(e) => {
            error!("Database error: {}", e);
            return false;
        }
    }
    true
}

#[derive(sqlx::FromRow)]
struct Ephemeral {
    id: i64,
    url: String,
    ip: String,
}
