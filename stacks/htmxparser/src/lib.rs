use gtk::{Widget, prelude::*};
use roxmltree::{Children, Document, Node, NodeType};

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
    let tree = match Document::parse(&markup) {
        Ok(tree) => tree,
        Err(e) => {
            let label = gtk::Label::builder()
                .label(format!("Error parsing HTML! {e}"))
                .vexpand(true)
                .build();
            webview.append(&label);
            return webview;
        }
    };
    for element in tree.root_element().children() {
        if let Some(element) = process_element(element) {
            webview.append(&element);
        } else {
            println!("Unrecognised element: {element:#?}");
        }
    }
    webview
}

fn process_element(element: Node) -> Option<Widget> {
    let kind = derive_kind(element.tag_name().name())?;
    match kind {
        ElemKind::Label(text) => process_text(text, element.children()),
    }
}

fn derive_kind(name: &str) -> Option<ElemKind> {
    Some(match name {
        "h1" => ElemKind::Label(Text::Title(Header::Level1)),
        "h2" => ElemKind::Label(Text::Title(Header::Level2)),
        "h3" => ElemKind::Label(Text::Title(Header::Level3)),
        "h4" => ElemKind::Label(Text::Title(Header::Level4)),
        "h5" => ElemKind::Label(Text::Title(Header::Level5)),
        "h6" => ElemKind::Label(Text::Title(Header::Level6)),
        "i" => ElemKind::Label(Text::Format(Style::Italic)),
        "b" => ElemKind::Label(Text::Format(Style::Bold)),
        "u" => ElemKind::Label(Text::Format(Style::Underline)),
        "s" => ElemKind::Label(Text::Format(Style::Strikethrough)),
        "sup" => ElemKind::Label(Text::Format(Style::Superscript)),
        "sub" => ElemKind::Label(Text::Format(Style::Subscript)),
        "code" => ElemKind::Label(Text::Format(Style::Code)),
        _ => return None,
    })
}

fn process_text(kind: Text, children: Children) -> Option<Widget> {
    let label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .use_markup(true)
        .build();
    let text = process_texts(kind, children)?;
    label.set_label(&text);
    Some(label.into())
}

fn process_texts(kind: Text, children: Children) -> Option<String> {
    let mut markup = String::new();
    match kind {
        Text::Title(header) => markup.push_str(&process_header(header, children)?),
        Text::Format(style) => markup.push_str(&process_style(style, children)?),
    }
    Some(markup)
}

fn process_header(kind: Header, children: Children) -> Option<String> {
    let mut markup = String::new();
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let Some(ElemKind::Label(Text::Format(style))) =
                    derive_kind(child.tag_name().name())
                {
                    if let Some(data) = process_style(style, child.children()) {
                        markup.push_str(&data);
                    }
                }
            }
            NodeType::Text => {
                if let Some(text) = child.text() {
                    markup.push_str(&escape(text));
                }
            }
            _ => {}
        }
    }
    match kind {
        Header::Level1 => markup = format!("<span size='xx-large'>{markup}</span>"),
        Header::Level2 => markup = format!("<span size='x-large'>{markup}</span>"),
        Header::Level3 => markup = format!("<span size='large'>{markup}</span>"),
        Header::Level4 => markup = format!("<span>{markup}</span>"),
        Header::Level5 => markup = format!("<span>{markup}</span>"),
        Header::Level6 => markup = format!("<span>{markup}</span>"),
    }
    Some(markup)
}

fn process_style(kind: Style, children: Children) -> Option<String> {
    let mut markup = String::new();
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let Some(ElemKind::Label(Text::Format(style))) =
                    derive_kind(child.tag_name().name())
                {
                    if let Some(data) = process_style(style, child.children()) {
                        markup.push_str(&data);
                    }
                }
            }
            NodeType::Text => {
                if let Some(text) = child.text() {
                    markup.push_str(&escape(text));
                }
            }
            _ => {}
        }
    }
    match kind {
        Style::Bold => markup = format!("<b>{markup}</b>"),
        Style::Italic => markup = format!("<i>{markup}</i>"),
        Style::Underline => markup = format!("<u>{markup}</u>"),
        Style::Strikethrough => markup = format!("<s>{markup}</s>"),
        Style::Superscript => markup = format!("<sup>{markup}</sup>"),
        Style::Subscript => markup = format!("<sub>{markup}</sub>"),
        Style::Code => markup = format!("<tt>{markup}</tt>"),
    }
    Some(markup)
}

fn escape(line: &str) -> String {
    line.replace("&", "&amp;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
        .replace("'", "&apos;")
        .replace("\"", "&quot;")
}

enum ElemKind {
    Label(Text),
}

enum Text {
    Title(Header),
    Format(Style),
}

enum Header {
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
    Level6,
}

enum Style {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Superscript,
    Subscript,
    Code,
}
