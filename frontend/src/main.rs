use gtk::{
    Application, ApplicationWindow,
    glib::{self, clone},
    prelude::*,
};
use std::process::Command;
const APP_ID: &str = "dither.browser";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
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
    entry.connect_activate(clone!(
        #[weak]
        label,
        move |entry| {
            let output = Command::new("./target/debug/url_resolver")
                .arg("-r")
                .arg(entry.text())
                .output()
                .unwrap(); //Never unwrap lolz gonna have to remember to remove this
            label.set_text(
                str::from_utf8(&output.stdout)
                    .unwrap()
                    .lines()
                    .last()
                    .unwrap_or_default(),
            );
        }
    ));
    search_bar.set_child(Some(&entry));
    window.present();
}
