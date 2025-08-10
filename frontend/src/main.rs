use async_std::io;
use gtk::{
    Application, ApplicationWindow, gdk,
    glib::{self, clone},
    prelude::*,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use std::{env, fs, net::TcpStream, path};
use tracing::{debug, error, info, trace, warn};
use url_resolver::{dns_task, get_stack_info, parse_stack, resolve};
use utils::{
    fqdn_to_upe, get_config_dir, receive_data, send_data, sql_cols, status, trace_subscription,
};
const APP_ID: &str = "dither.browser";
const PROJ_NAME: &str = "Browser";

#[async_std::main]
async fn main() -> glib::ExitCode {
    let mut verbose_level = 0u8;
    let mut caching = true;
    let mut force_stacks_refresh = false;
    let mut stacks = String::new();
    let args: Vec<String> = env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "no-caching" => caching = false,
                "refresh-stacks" => force_stacks_refresh = true,
                "stacks" => stacks = args[i + 1].clone(),
                "verbose" => verbose_level += 1,
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'c' => caching = false,
                    'r' => force_stacks_refresh = true,
                    'S' => {
                        stacks = args[argindex + 1].clone();
                        argindex += 1;
                    }
                    's' => {
                        stacks += &args[argindex + 1].clone();
                        argindex += 1;
                    }
                    'v' => verbose_level += 1,
                    _ => panic!("Pre-init failure; unknown short-name argument: {arg}"),
                }
            }
        }
    }
    trace_subscription(verbose_level);
    if !compile_stacks(force_stacks_refresh).await {
        return glib::ExitCode::FAILURE;
    }
    if stacks.is_empty() {
        stacks = list_stacks().await;
    }
    if stacks.is_empty() {
        error!("No stacks found!");
        return glib::ExitCode::FAILURE;
    }
    debug!("Using stacks: {stacks}");
    if caching {
        caching = create_cache().await;
    }
    debug!("Caching enabled: {caching}");
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, caching, stacks.clone()));
    app.run_with_args(&[""])
}

fn build_ui(app: &Application, caching: bool, stacks: String) {
    load_css();
    let mainbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    let webview = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .min_content_width(400)
        .min_content_height(300)
        .build();
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Browser")
        .default_height(600)
        .default_width(1000)
        .build();
    let label = gtk::Label::builder()
        .label("Enter URL")
        .vexpand(true)
        .css_classes(["title-1"])
        .build();
    let header_bar = gtk::HeaderBar::new();
    let search_button = gtk::ToggleButton::new();
    search_button.set_icon_name("system-search-symbolic");
    header_bar.pack_end(&search_button);
    let search_bar = gtk::SearchBar::builder()
        .valign(gtk::Align::Start)
        .key_capture_widget(&window)
        .build();
    search_bar.set_css_classes(&[""]);
    unsafe {
        search_bar.set_data("page-content", PageContent::Nothing);
    }
    webview.append(&label);
    scrolled_window.set_child(Some(&webview));
    mainbox.append(&search_bar);
    mainbox.append(&scrolled_window);
    window.set_child(Some(&mainbox));
    window.set_titlebar(Some(&header_bar));
    search_button
        .bind_property("active", &search_bar, "search-mode-enabled")
        .sync_create()
        .bidirectional()
        .build();
    let entry = gtk::SearchEntry::new();
    search_bar.set_child(Some(&entry));
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
    let sb_clone = search_bar.clone();
    entry.connect_activate(move |_| {
        let scrolled_window_weak: glib::WeakRef<gtk::ScrolledWindow> =
            gtk::ScrolledWindow::downgrade(&scrolled_window);
        let pagecontent;
        unsafe {
            pagecontent = sb_clone.steal_data("page-content");
        }
        glib::MainContext::default().spawn_local(async move {
            if let Some(scrolledwindow) = scrolled_window_weak.upgrade() {
                if let Some(pagecontent) = pagecontent {
                    match pagecontent {
                        PageContent::Page(pagedata) => {
                            scrolledwindow.set_child(Some(&pagedata.0));
                        }
                        PageContent::Status(err) => {
                            scrolledwindow.set_child(Some(&no_webpage(err)));
                        }
                        PageContent::Failure(e) => {
                            error!("FS error: {}", e);
                            scrolledwindow.set_child(Some(&no_webpage(status::SHAT_THE_BED)));
                        }
                        _ => {}
                    }
                }
            }
        });
    });
    entry.connect_changed(move |entry| {
        let entry_clone = entry.clone();
        let stacks_clone = stacks.clone();
        let searchbar_weak = gtk::SearchBar::downgrade(&search_bar);
        glib::MainContext::default().spawn_local(async move {
            if let Some(searchbar) = searchbar_weak.upgrade() {
                searchbar.set_css_classes(&[""]);
                let buffer = try_get_webpage(&entry_clone, caching, &stacks_clone).await;
                match &buffer {
                    PageContent::Page(pagedata) => match pagedata.1 {
                        status::SUCCESS => {
                            searchbar.set_css_classes(&["greensearch"]);
                        }
                        _ => {
                            searchbar.set_css_classes(&["redsearch"]);
                        }
                    },
                    PageContent::Nothing => {}
                    _ => {
                        searchbar.set_css_classes(&["redsearch"]);
                    }
                }
                unsafe {
                    searchbar.set_data("page-content", buffer);
                }
            }
        });
    });
    window.present();
}

fn load_css() {
    let display = match gdk::Display::default() {
        Some(display) => display,
        None => {
            error!("Couldn't connect to GDK display!");
            return;
        }
    };
    let provider = gtk::CssProvider::new();
    let priority = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
    provider.load_from_bytes(&glib::Bytes::from(
        br#"
    .redsearch   text {color: #d41818ff;}
    .greensearch text {color: #00a900;}"#,
    ));
    gtk::style_context_add_provider_for_display(&display, &provider, priority);
}

async fn try_get_webpage(entry: &gtk::SearchEntry, caching: bool, stacks: &str) -> PageContent {
    if entry.text().is_empty() {
        return PageContent::Nothing;
    }
    let mut statuscode = status::HOST_UNREACHABLE;
    let (url, port, endpoint) = fqdn_to_upe(&entry.text());
    let mut webview = None;
    trace!("URL: {url}, Port: {port:?}, Endpoint: {endpoint}");
    if !caching {
        trace!("Caching disabled. Will resolve directly.");
        let res = resolve_url(&url).await;
        statuscode = res.1;
        if let Some(ip) = res.0 {
            webview = Some(draw_webpage((ip, endpoint), stacks));
        }
    } else {
        let config_dir = match get_config_dir(PROJ_NAME) {
            Some(dir) => dir,
            None => {
                return PageContent::Failure(io::Error::new(
                    io::ErrorKind::NotSeekable,
                    "Could not determine compatable configuration directory.",
                ));
            }
        };
        match fs::create_dir_all(&config_dir) {
            Ok(_) => {}
            Err(e) => {
                return PageContent::Failure(io::Error::other(format!("FS error: {e}")));
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
                return PageContent::Failure(io::Error::other(format!("Database error: {e}")));
            }
        };
        trace!("Successfully connected to database");
        let mut blocks = url.split('.').collect::<Vec<_>>();
        let mut lookahead = String::new();
        let mut verified_url = None;
        let mut cache_used = false;
        while !blocks.is_empty() {
            trace!("Blocks: {:?}", blocks);
            let sub_url = blocks.join(".");
            if let Ok(Some(record)) = sqlx::query_as::<_, sql_cols::EphemeralRecord>(
                "SELECT * FROM ephemeral WHERE url = ?",
            )
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
                    let res = dns_task(&ip, &lookahead).await;
                    res.0
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
                        webview = Some(draw_webpage((dest.clone(), endpoint.clone()), stacks));
                        verified_url = Some(dest.clone());
                        let res = resolve_url(&entry.text()).await;
                        statuscode = res.1;
                        if let Some(validated_url) = res.0 {
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
                                webview = Some(draw_webpage(
                                    (validated_url.clone(), endpoint.clone()),
                                    stacks,
                                ));
                                verified_url = Some(validated_url);
                            }
                        };
                    }
                    None => {
                        error!("Congrats! I have no idea how you got here.");
                        let res = resolve_url(&entry.text()).await;
                        statuscode = res.1;
                        verified_url = res.0;
                        if let Some(dest) = &verified_url {
                            webview =
                                Some(draw_webpage((dest.to_string(), endpoint.clone()), stacks));
                        }
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
            let res = resolve_url(&entry.text()).await;
            statuscode = res.1;
            verified_url = res.0;
            if let Some(dest) = &verified_url {
                webview = Some(draw_webpage((dest.to_string(), endpoint), stacks));
            }
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
    }
    if let Some(webview) = webview {
        let view = webview.await;
        statuscode = view.1;
        if let Some(webview) = view.0 {
            return PageContent::Page((webview, statuscode));
        }
    }
    PageContent::Status(statuscode)
}

async fn resolve_url(destination: &str) -> (Option<String>, u32) {
    let ip = resolve(destination, None, None, None).await;
    if ip.0.is_empty() {
        (None, ip.1)
    } else {
        (Some(ip.0), ip.1)
    }
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

async fn compile_stacks(force: bool) -> bool {
    let config_dir = match get_config_dir(PROJ_NAME) {
        Some(dir) => dir,
        None => {
            error!("Could not determine compatable configuration directory.");
            return false;
        }
    };
    if let Ok(app_loc) = std::env::current_exe() {
        if let Some(app_dir) = app_loc.parent() {
            let dbpath = config_dir.join(path::Path::new("stacks.db"));
            if dbpath.exists() {
                if !force {
                    trace!("Found stacks DB!");
                    return true;
                }
                warn!("Overwriting stacks DB");
                match fs::remove_file(&dbpath) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("FS error: {}", e);
                        return false;
                    }
                };
            }
            debug!("Generating stacks database...");
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
                CREATE TABLE IF NOT EXISTS stacks
                    (id INTEGER PRIMARY KEY AUTOINCREMENT,
                    stack TEXT UNIQUE NOT NULL,
                    library TEXT UNIQUE NOT NULL);
                "#,
            )
            .execute(&pool)
            .await
            {
                Ok(_) => {
                    let files = match fs::read_dir(app_dir) {
                        Ok(dir) => dir,
                        Err(e) => {
                            error!("FS error: {}", e);
                            return false;
                        }
                    };
                    for file in files.flatten() {
                        let filerelname = file.file_name().to_string_lossy().to_string();
                        let filefullname = file
                            .path()
                            .canonicalize()
                            .unwrap_or(file.path())
                            .to_string_lossy()
                            .to_string();
                        if filerelname.starts_with(std::env::consts::DLL_PREFIX)
                            && filerelname.ends_with(std::env::consts::DLL_SUFFIX)
                        {
                            if let Some(stack) = get_stack_info(&file.path()) {
                                match sqlx::query(
                                    r#"INSERT INTO stacks (stack, library) VALUES (?, ?)"#,
                                )
                                .bind(&stack)
                                .bind(&filefullname)
                                .execute(&pool)
                                .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("Database error: {}", e);
                                        return false;
                                    }
                                };
                            }
                        }
                    }
                    return true;
                }
                Err(e) => {
                    error!("Database error: {}", e);
                }
            }
        }
    }
    error!("Undocumented failure.");
    false
}

async fn list_stacks() -> String {
    let mut stacks = String::new();
    let config_dir = match get_config_dir(PROJ_NAME) {
        Some(dir) => dir,
        None => {
            error!("Could not determine compatable configuration directory.");
            return stacks;
        }
    };
    let dbpath = config_dir.join(path::Path::new("stacks.db"));
    if !dbpath.exists() {
        error!("Missing stacks DB!");
        return stacks;
    }
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
            return stacks;
        }
    };
    if let Ok(record) = sqlx::query_as::<_, sql_cols::StackRecord>("SELECT stack FROM stacks")
        .fetch_all(&pool)
        .await
    {
        for stack in record {
            stacks += &stack.stack;
        }
    };
    stacks
}

async fn draw_webpage(address: (String, String), stacks: &str) -> (Option<gtk::Box>, u32) {
    let res = get_data(&address, stacks);
    match res.0 {
        Some(data) => (
            parse_stack(&String::from_utf8_lossy(&data.0), &data.1, PROJ_NAME).await,
            res.1,
        ),
        None => (None, res.1),
    }
}

fn no_webpage(err: u32) -> gtk::Box {
    let label = gtk::Label::builder()
        .label(format!(
            "Error retrieving webpage! {}: {}",
            err,
            status::decode(&err)
        ))
        .vexpand(true)
        .css_classes(["title-1", "warning"])
        .build();
    let webview = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    webview.append(&label);
    webview
}

fn get_data(address: &(String, String), stacks: &str) -> (Option<(Vec<u8>, String)>, u32) {
    let mut statuscode = status::HOST_UNREACHABLE;
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
    let Ok(stream) = TcpStream::connect(&address.0) else {
        error!("Failed to connect to {}!", &address.0);
        return (None, statuscode);
    };
    let mut payload = program_version[0].to_le_bytes().to_vec();
    payload.extend_from_slice(&program_version[1].to_le_bytes());
    payload.extend_from_slice(&program_version[2].to_le_bytes());
    payload.extend_from_slice(stacks.as_bytes());
    payload.extend_from_slice("/".as_bytes());
    payload.extend_from_slice(address.1.as_bytes());
    send_data(&payload, &stream);
    let response = receive_data(&stream);
    match response.len() {
        4 => {
            let code = u32::from_le_bytes(response.try_into().unwrap());
            error!("{}", &status::decode(&code));
            statuscode = code;
        }
        0..9 => {
            error!("Server send an invalid response.");
            return (None, status::BAD_RESPONSE);
        }
        _ => {
            let code = u32::from_le_bytes(response[0..4].try_into().unwrap());
            match code {
                status::SUCCESS => {
                    let stack = String::from_utf8_lossy(&response[4..9]).to_string();
                    info!("Server responsed with protocol {}", stack);
                    return (Some((response[9..].to_vec(), stack)), code);
                }
                x => {
                    error!("{}", &status::decode(&x));
                    statuscode = x;
                }
            }
        }
    }
    (None, statuscode)
}

enum PageContent {
    Page((gtk::Box, u32)),
    Status(u32),
    Failure(io::Error),
    Nothing,
}
