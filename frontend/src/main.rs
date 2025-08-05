use gtk::{
    Application, ApplicationWindow,
    glib::{self, clone},
    prelude::*,
};
use sqlx::{
    Pool,
    sqlite::{SqliteConnectOptions, SqlitePool},
};
use std::{env, path};
use url_resolver::resolve;
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
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run_with_args(&[""])
}

fn build_ui(app: &Application) {
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
                try_cache_webpage(&entry_clone, &label).await;
            }
        });
    });
    search_bar.set_child(Some(&entry));
    window.present();
}

async fn try_cache_webpage(entry: &gtk::SearchEntry, label: &gtk::Label) {
    label.set_text("Loading...");
    let config_dir = if let Some(config_dir) = get_config_dir(PROJ_NAME) {
        config_dir
    } else {
        label.set_text(
            &resolve_url(&entry.text())
                .await
                .unwrap_or("Website not found.".to_string()),
        );
        return;
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
            label.set_text(&e.to_string());
            return;
        }
    };
    // let mut lines = Vec::new();
    // let mut result = String::new();
    // file.read_to_end(&mut lines);
    // let mut found = false;
    // let mut lines: Vec<String> = String::from_utf8_lossy(&lines)
    //     .split('\n')
    //     .map(|s| s.to_owned())
    //     .collect();
    // for line in &lines {
    //     let split = line.split_once('\x00').unwrap_or_default();
    //     if split.0 == entry.text() {
    //         label.set_text(split.1);
    //         result = split.1.to_string();
    //         found = true;
    //         break;
    //     }
    // }
    // let ip = resolve_url(&entry.text()).await;
    // if let Some(ip) = ip {
    //     if ip != result {
    //         label.set_text(&ip);
    //         if !found {
    //             let line = entry.text().to_string() + "\x00" + &ip;
    //             lines.push(line);
    //         } else {
    //             let line = entry.text().to_string() + "\x00";
    //             let index = &lines.iter().enumerate().find(|s| s.1.starts_with(&line));
    //             if index.is_some() {
    //                 let line = line + &ip;
    //                 lines[index.unwrap().0] = line;
    //             }
    //         }
    //         file.write_all(lines.concat().as_bytes());
    //     }
    // }
}

async fn resolve_url(destination: &str) -> Option<String> {
    let ip = resolve(destination, None, None, None).await;
    if ip.is_empty() { None } else { Some(ip) }
}
