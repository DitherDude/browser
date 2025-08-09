use gtk::prelude::*;

#[unsafe(no_mangle)]
pub fn get_elements(markup: String) -> gtk::Box {
    let _ = gtk::init();
    let webview = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    for line in markup.lines() {
        let child = process_child(line, None);
        webview.append(&child);
    }
    webview
}

fn process_child(markup: &str, child: Option<gtk::Widget>) -> gtk::Widget {
    match child {
        None => match markup.split_ascii_whitespace().next().unwrap_or_default() {
            "#" => {
                let header1 = gtk::Label::builder()
                    .label(markup.trim_start_matches("# "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-1"])
                    .build();
                header1.into()
            }
            "##" => {
                let header2 = gtk::Label::builder()
                    .label(markup.trim_start_matches("## "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-2"])
                    .build();
                header2.into()
            }
            "###" => {
                let header3 = gtk::Label::builder()
                    .label(markup.trim_start_matches("### "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-3"])
                    .build();
                header3.into()
            }
            "####" => {
                let header4 = gtk::Label::builder()
                    .label(markup.trim_start_matches("#### "))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["title-4"])
                    .build();
                header4.into()
            }
            _ => {
                let para = gtk::Label::builder()
                    .label(markup)
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(["document"])
                    .build();
                para.into()
            }
        },
        Some(child) => {
            println!("EEEEE");
            child
        }
    }
}

#[unsafe(no_mangle)]
pub fn stacks() -> String {
    "MRKDN".to_owned()
}
