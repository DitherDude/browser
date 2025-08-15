use fancy_regex::Regex;
use gtk::prelude::*;

const MAIN_MATCH: &str = r"\<X.*?\>.*?\<\/X\>";

#[unsafe(no_mangle)]
pub fn stacks() -> String {
    "HTMLX".to_owned()
}

#[unsafe(no_mangle)]
pub fn get_elements(markup: String) -> gtk::Box {
    let _ = gtk::init();
    let webview = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    let children = get_children(&markup);
    for child in children {
        webview.append(&child);
    }
    webview
}

fn get_children(markup: &str) -> Vec<gtk::Widget> {
    let markup = markup.to_owned();
    let mut widgets = Vec::new();
    let (children, _markup) = handle_children(&markup, "h1", ElementType::Label);
    widgets.extend_from_slice(&children);
    widgets
}

fn handle_children(markup: &str, attr: &str, elemtype: ElementType) -> (Vec<gtk::Widget>, String) {
    let mut children = Vec::new();
    let markup = markup.to_owned();
    let attr_re = match Regex::new(&MAIN_MATCH.replace("X", attr)) {
        Ok(regex) => regex,
        Err(_) => {
            return (children, markup);
        }
    };
    if let Ok(Some(captures)) = attr_re.captures(&markup.clone()) {
        for capture in captures.iter().flatten() {
            let trim = capture.as_str();
            let trim = trim.trim_start_matches(&format!("<{attr}>"));
            let trim = trim.trim_end_matches(&format!("</{attr}>"));
            match elemtype {
                ElementType::Label => {
                    if let Some(child) = process_label(trim, attr) {
                        children.push(child);
                    }
                }
                _ => {
                    //TODO
                }
            }
        }
    }
    (children, markup)
}

fn process_label(body: &str, attr: &str) -> Option<gtk::Widget> {
    let label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .use_markup(true)
        .build();
    let body = escape(body);
    match attr {
        "h1" => label.set_label(&format!("<span size='xx-large'>{body}</span>")),
        _ => return None,
    }
    Some(label.into())
}

fn escape(line: &str) -> String {
    line.replace("&", "&amp;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
        .replace("'", "&apos;")
        .replace("\"", "&quot;")
}

enum ElementType {
    _Div,
    Label,
}
