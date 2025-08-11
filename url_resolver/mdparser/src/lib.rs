use fancy_regex::Regex;
use gtk::prelude::*;

const MOD_MATCH: &str = r"(?<!\\)(?:\\\\)*(X.+?)(?<!\\)X";
const HEAD_MATCH: &str = r"^(?<!\\)X.*";

#[unsafe(no_mangle)]
pub fn get_elements(markup: String) -> gtk::Box {
    let _ = gtk::init();
    let webview = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    for line in markup.lines() {
        let safe_line = escape(line);
        webview.append(&process_headers(&safe_line));
    }
    webview
}

fn escape(line: &str) -> String {
    line.replace("&", "&amp;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
        .replace("'", "&apos;")
        .replace("\"", "&quot;")
}

fn process_headers(markup: &str) -> gtk::Widget {
    let label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .use_markup(true)
        .build();
    let mut markup = markup.to_owned();
    let header_re = match Regex::new(&HEAD_MATCH.replace("X", "#")) {
        Ok(regex) => regex,
        Err(_) => {
            return label.into();
        }
    };
    let result = header_re.is_match(&markup);
    if result.unwrap_or(false) {
        if markup.starts_with("# ") {
            markup = process_header(&markup, "# ", "<span size='xx-large'>", "</span>");
        } else if markup.starts_with("## ") {
            markup = process_header(&markup, "## ", "<span size='x-large'>", "</span>");
        } else if markup.starts_with("### ") {
            markup = process_header(&markup, "### ", "<span size='large'>", "</span>");
        }
        return done(&markup, label).into();
    };
    let footer_re = match Regex::new(&HEAD_MATCH.replace("X", "-")) {
        Ok(regex) => regex,
        Err(_) => {
            return label.into();
        }
    };
    let result = footer_re.is_match(&markup);
    if result.unwrap_or(false) {
        if markup.starts_with("-# ") {
            markup = process_header(&markup, "-# ", "<span size='small'>", "</span>")
        } else if markup.starts_with("--# ") {
            markup = process_header(&markup, "--# ", "<span size='x-small'>", "</span>")
        }
        return done(&markup, label).into();
    }
    markup = process_attributes(&markup);
    label.set_label(&markup.replace("\\\\", "\\"));
    label.into()
}

fn process_header(markup: &str, attr: &str, head: &str, tail: &str) -> String {
    let mut result = markup.to_owned();
    let newmarkup = result.split_once(attr).unwrap_or(("", &result)).1;
    result = process_attributes(&format!("{head}{newmarkup}{tail}"));
    result
}

fn done(markup: &str, label: gtk::Label) -> gtk::Label {
    let markup = process_attributes(markup);
    label.set_label(&markup.replace("\\\\", "\\"));
    label
}
fn process_attributes(markup: &str) -> String {
    let mut markup = markup.to_owned();
    markup = process_attribute(&markup, r"\*\*", 2, "<b>", "</b>");
    markup = process_attribute(&markup, r"~~", 2, "<s>", "</s>");
    markup = process_attribute(&markup, r"==", 2, "<u>", "</u>");
    markup = process_attribute(&markup, r"\*", 1, "<i>", "</i>");
    markup = process_attribute(&markup, r"_", 1, "<i>", "</i>");
    markup = process_attribute(&markup, r"`", 1, "<tt>", "</tt>");
    markup
}

fn process_attribute(markup: &str, attr: &str, len: usize, head: &str, tail: &str) -> String {
    let mut markup = markup.to_owned();
    let attr_re = match Regex::new(&MOD_MATCH.replace("X", attr)) {
        Ok(regex) => regex,
        Err(_) => {
            return markup;
        }
    };
    if let Ok(Some(captures)) = attr_re.captures(&markup.clone()) {
        for capture in captures.iter() {
            match capture {
                Some(result) => {
                    let trim = result.as_str().to_string();
                    let trim = trim[len..trim.as_str().len() - len].to_owned();
                    markup = markup.replace(result.as_str(), &format!("{head}{trim}{tail}"));
                }
                _ => (),
            }
        }
    }
    markup
}

#[unsafe(no_mangle)]
pub fn stacks() -> String {
    "MRKDN".to_owned()
}
