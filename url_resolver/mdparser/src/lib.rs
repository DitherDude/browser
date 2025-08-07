use gtk::prelude::*;
use std::{env, fs::File, io::Read};
use tracing::{Level, error};

pub fn main() {
    let mut verbose_level = 0u8;
    let args: Vec<String> = env::args().collect();
    let mut filename = String::new();
    for (i, arg) in args.iter().enumerate() {
        if arg.starts_with("--") {
            match arg.strip_prefix("--").unwrap_or_default() {
                "verbose" => verbose_level += 1,
                "filename" => filename = args[i + 1].clone(),
                _ => panic!("Pre-init failure; unknown long-name argument: {arg}"),
            }
        } else if arg.starts_with("-") {
            let mut argindex = i;
            for char in arg.strip_prefix("-").unwrap_or_default().chars() {
                match char {
                    'v' => verbose_level += 1,
                    'f' => {
                        filename = args[argindex + 1].clone();
                        argindex += 1;
                    }
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
    let mut contents = String::new();
    match File::open(&filename) {
        Ok(mut f) => match f.read_to_string(&mut contents) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to read file: {}", e);
                return;
            }
        },
        Err(e) => {
            error!("Failed to open file: {}", e);
            return;
        }
    };
    let _elements = get_elements(contents);
}

#[unsafe(no_mangle)]
pub fn get_elements(contents: String) -> gtk::Box {
    let _ = gtk::init();
    let mut contents = contents.clone();
    let widgets = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    while !contents.is_empty() {
        match contents.split_ascii_whitespace().next().unwrap_or_default() {
            "#" => {
                let header = contents.split_once('\n').unwrap_or_default().0;
                let header = gtk::Label::builder()
                    .label(header.trim_start_matches("# "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-1"])
                    .build();
                widgets.append(&header);
                contents = contents.split_once('\n').unwrap_or_default().1.to_owned();
            }
            "##" => {
                let header = contents.split_once('\n').unwrap_or_default().0;
                let header = gtk::Label::builder()
                    .label(header.trim_start_matches("## "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-2"])
                    .build();
                widgets.append(&header);
                contents = contents.split_once('\n').unwrap_or_default().1.to_owned();
            }
            _ => {
                let para = contents.split_once('\n').unwrap_or_default().0;
                let para = gtk::Label::builder()
                    .label(para)
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["document"])
                    .build();
                widgets.append(&para);
                contents = contents.split_once('\n').unwrap_or_default().1.to_owned();
            }
        }
    }
    widgets
}

#[unsafe(no_mangle)]
pub fn test() -> String {
    "Working!".to_owned()
}
