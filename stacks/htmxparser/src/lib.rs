use gtk::{Widget, prelude::*};
use roxmltree::{Attributes, Children, Document, Node, NodeType};

#[unsafe(no_mangle)]
pub fn stacks() -> String {
    "HTMLX".to_owned()
}

#[unsafe(no_mangle)]
pub fn get_elements(markup: String) -> gtk::Box {
    let mut markup = markup;
    if !markup.starts_with("<xml") {
        markup = format!("<xml>{markup}</xml>");
    }
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
        if element.node_type() == NodeType::Text
            && element.text().is_some_and(|x| x.trim().is_empty())
        {
            continue;
        }
        if let Some(element) = process_element(&element) {
            webview.append(&element);
        } else {
            println!("Unrecognised element: {element:#?}");
        }
    }
    webview
}

fn derive_kind(name: &str) -> ElemKind {
    match name {
        "h1" => ElemKind::Label(Text::Kind(TextKind::Header1)),
        "h2" => ElemKind::Label(Text::Kind(TextKind::Header2)),
        "h3" => ElemKind::Label(Text::Kind(TextKind::Header3)),
        "h4" => ElemKind::Label(Text::Kind(TextKind::Header4)),
        "h5" => ElemKind::Label(Text::Kind(TextKind::Header5)),
        "h6" => ElemKind::Label(Text::Kind(TextKind::Header6)),
        "p" => ElemKind::Label(Text::Kind(TextKind::Normal)),
        "i" => ElemKind::Label(Text::Style(TestStyle::Italic)),
        "b" => ElemKind::Label(Text::Style(TestStyle::Bold)),
        "u" => ElemKind::Label(Text::Style(TestStyle::Underline)),
        "s" => ElemKind::Label(Text::Style(TestStyle::Strikethrough)),
        "sup" => ElemKind::Label(Text::Style(TestStyle::Superscript)),
        "sub" => ElemKind::Label(Text::Style(TestStyle::Subscript)),
        "code" => ElemKind::Label(Text::Style(TestStyle::Code)),
        "grid" => ElemKind::Grid(GridKind::Grid),
        "griditem" | "gi" => ElemKind::Grid(GridKind::GridItem),
        "div" | "box" => ElemKind::Container,
        _ => ElemKind::Fallback,
    }
}

fn process_element(element: &Node) -> Option<Widget> {
    let kind = derive_kind(element.tag_name().name());
    match kind {
        ElemKind::Label(kind) => process_label(&kind, element.children(), element.attributes()),
        ElemKind::Grid(GridKind::Grid) => process_grid(element.children(), element.attributes()),
        ElemKind::Grid(GridKind::GridItem) => None,
        ElemKind::Container => process_container(element.children(), element.attributes()),
        ElemKind::Fallback => element
            .text()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|text| {
                gtk::Label::builder()
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .label(text.trim())
                    .build()
                    .into()
            }),
    }
}

/* #region Label */
fn process_label(kind: &Text, children: Children, attributes: Attributes) -> Option<Widget> {
    let (text, defaults) = process_text(kind, children, attributes)?;
    let label = gtk::Label::builder()
        .halign(defaults.halign)
        .valign(defaults.valign)
        .hexpand(defaults.hexpand)
        .vexpand(defaults.vexpand)
        .use_markup(true)
        .build();
    label.set_label(&text);
    Some(label.into())
}

fn process_text(
    kind: &Text,
    children: Children,
    attributes: Attributes,
) -> Option<(String, WidgetDefaults)> {
    let mut markup = String::new();
    let mut defaults = WidgetDefaults::new();
    match kind {
        Text::Kind(header) => {
            let (kind, default) = text_kind(header, children, attributes)?;
            defaults = default;
            markup.push_str(&kind);
        }
        Text::Style(style) => markup.push_str(&text_style(style, children)?),
    }
    Some((markup, defaults))
}

fn text_kind(
    kind: &TextKind,
    children: Children,
    attributes: Attributes,
) -> Option<(String, WidgetDefaults)> {
    let mut markup = String::new();
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let ElemKind::Label(Text::Style(style)) = derive_kind(child.tag_name().name()) {
                    if let Some(data) = text_style(&style, child.children()) {
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
    let (attributes, defaults) = text_attributes(attributes);
    match kind {
        TextKind::Header1 => markup = format!("<span size='xx-large' {attributes}>{markup}</span>"),
        TextKind::Header2 => markup = format!("<span size='x-large' {attributes}>{markup}</span>"),
        TextKind::Header3 => markup = format!("<span size='large' {attributes}>{markup}</span>"),
        TextKind::Header4 => markup = format!("<span size='medium' {attributes}>{markup}</span>"),
        TextKind::Header5 => markup = format!("<span size='small' {attributes}>{markup}</span>"),
        TextKind::Header6 => markup = format!("<span size='x-small' {attributes}>{markup}</span>"),
        TextKind::Normal => markup = format!("<span {attributes}>{markup}</span>"),
    }
    Some((markup, defaults))
}

fn text_attributes(attributes: Attributes) -> (String, WidgetDefaults) {
    let mut markup = String::new();
    let mut defaults = WidgetDefaults::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "font" => markup.push_str(&format!("font='{val}' ")),
            "ff" | "font_family" | "face" => markup.push_str(&format!("face='{val}' ")),
            "size" => {
                if gtk::pango::FontDescription::from_string(val).size() != 0 {
                    markup.push_str(&format!("size='{val}' "));
                }
            }
            "style" => match val {
                "o" | "oblique" => markup.push_str("style='oblique' "),
                "i" | "italic" => markup.push_str("style='italic' "),
                _ => markup.push_str("style='normal' "),
            },
            "weight" => match val {
                "ul" | "ultralight" => markup.push_str("weight='ultralight' "),
                "l" | "light" => markup.push_str("weight='light' "),
                "b" | "bold" => markup.push_str("weight='bold' "),
                "ub" | "ultrabold" => markup.push_str("weight='ultrabold' "),
                "h" | "heavy" => markup.push_str("weight='heavy' "),
                _ => markup.push_str("weight='normal' "),
            },
            "variant" => match val {
                "sc" | "small-caps" | "small_caps" | "smallcaps" => {
                    markup.push_str("variant='small-caps' ")
                }
                "asc" | "all-small-caps" | "all_small_caps" | "allsmallcaps" => {
                    markup.push_str("variant='all-small-caps' ")
                }
                "pc" | "petite-caps" | "petite_caps" | "petitecaps" => {
                    markup.push_str("variant='petite-caps' ")
                }
                "apc" | "all-petite-caps" | "all_petite_caps" | "allpetitecaps" => {
                    markup.push_str("variant='all-petite-caps' ")
                }
                "uc" | "unicase" => markup.push_str("variant='unicase' "),
                "tc" | "title-caps" | "title_caps" | "titlecaps" => {
                    markup.push_str("variant='title-caps' ")
                }
                _ => markup.push_str("variant='normal' "),
            },
            "stretch" => match val {
                "uc" | "ultracondensed" => markup.push_str("stretch='ultracondensed' "),
                "ec" | "extracondensed" => markup.push_str("stretch='extracondensed' "),
                "c" | "condensed" => markup.push_str("stretch='condensed' "),
                "sc" | "semicondensed" => markup.push_str("stretch='semicondensed' "),
                "se" | "semiexpanded" => markup.push_str("stretch='semiexpanded' "),
                "e" | "expanded" => markup.push_str("stretch='expanded' "),
                "ee" | "extraexpanded" => markup.push_str("stretch='extraexpanded' "),
                "ue" | "ultraexpanded" => markup.push_str("stretch='ultraexpanded' "),
                _ => markup.push_str("stretch='normal' "),
            },
            "font_features" | "features" => markup.push_str(&format!("font_features='{val}' ")),
            "foreground" | "fgcolor" | "color" => {
                if gtk::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("color='{val}' "))
                }
            }
            "background" | "bgcolor" => {
                if gtk::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("bgcolor='{val}' "))
                }
            }
            "alpha" | "fgalpha" => {
                if let Ok(val) = val.parse::<u16>() {
                    markup.push_str(&format!("alpha='{}' ", val as u32 + 1));
                } else if val
                    .strip_suffix("%")
                    .filter(|x| x.parse::<u8>().is_ok_and(|x| x <= 100))
                    .is_some()
                {
                    markup.push_str(&format!("alpha='{val}' "))
                }
            }
            "background_alpha" | "bgalpha" => {
                if let Ok(val) = val.parse::<u16>() {
                    markup.push_str(&format!("bgalpha='{}' ", val as u32 + 1));
                } else if val
                    .strip_suffix("%")
                    .filter(|x| x.parse::<u8>().is_ok_and(|x| x <= 100))
                    .is_some()
                {
                    markup.push_str(&format!("bgalpha='{val}' "))
                }
            }
            "underline" => match val {
                "s" | "single" => markup.push_str("underline='single' "),
                "d" | "double" => markup.push_str("underline='double' "),
                "l" | "low" => markup.push_str("underline='low' "),
                "e" | "error" => markup.push_str("underline='error' "),
                _ => markup.push_str("underline='none' "),
            },
            "ulc" | "underline_color" => {
                if gtk::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("underline_color='{val}' "))
                }
            }
            "overline" => match val {
                "s" | "single" => markup.push_str("overline='single' "),
                _ => markup.push_str("overline='none' "),
            },
            "olc" | "overline_color" => {
                if gtk::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("overline_color='{val}' "))
                }
            }
            "rise" => {
                if val.strip_suffix("pt").unwrap_or(val).parse::<i32>().is_ok() {
                    markup.push_str(&format!("rise='{val}' "));
                }
            }
            "baseline_shift" | "fall" => {
                if val.strip_suffix("pt").unwrap_or(val).parse::<i32>().is_ok() {
                    markup.push_str(&format!("baseline_shift='{val}' "));
                }
            }
            "font_scale" | "scale" => match val {
                "sup" | "superscript" => markup.push_str("font_scale='superscript' "),
                "sub" | "subscript" => markup.push_str("font_scale='subscript' "),
                "sc" | "small-caps" | "small_caps" | "smallcaps" => {
                    markup.push_str("font_scale='small-caps' ")
                }
                _ => {}
            },
            "s" | "strikethrough" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => markup.push_str("strikethrough='false' "),
                _ => markup.push_str("strikethrough='true' "),
            },
            "strikethrough_color" | "scolor" => {
                if gtk::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("strikethrough_color='{val}' "))
                }
            }
            "fallback" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => markup.push_str("fallback='false' "),
                _ => markup.push_str("fallback='true' "),
            },
            "lang" => markup.push_str(&format!("lang='{val}' ")),
            "letter_spacing" | "spacing" => {
                if val
                    .strip_suffix("pt")
                    .unwrap_or(val)
                    .parse::<f64>()
                    .is_ok_and(|x| x >= 0f64)
                {
                    markup.push_str(&format!("letter_spacing='{val}' "))
                }
            }
            "gravity" => match val {
                "south" | "bottom" => markup.push_str("gravity='south' "),
                "east" | "right" => markup.push_str("gravity='east' "),
                "north" | "top" => markup.push_str("gravity='north' "),
                "west" | "left" => markup.push_str("gravity='west' "),
                _ => markup.push_str("gravity='auto' "),
            },
            "gravity_hint" | "hint" => match val {
                "s" | "strong" => markup.push_str("gravity_hint='strong' "),
                "l" | "line" => markup.push_str("gravity_hint='line' "),
                _ => markup.push_str("gravity_hint='natural' "),
            },
            "show" => {
                if val
                    .split('|')
                    .all(|x| ["spaces", "line-breaks", "ignorables"].contains(&x))
                    || val == "none"
                {
                    markup.push_str(&format!("show='{val}' "));
                }
            }
            "insert_hyphens" | "hyphens" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => markup.push_str("insert_hyphens='false' "),
                _ => markup.push_str("insert_hyphens='true' "),
            },
            "allow_breaks" | "breaks" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => markup.push_str("allow_breaks='false' "),
                _ => markup.push_str("allow_breaks='true' "),
            },
            "line_height" | "height" => {
                if val
                    .strip_suffix("pt")
                    .is_some_and(|x| x.parse::<f64>().is_ok_and(|x| x >= 0f64))
                    || val.parse::<u16>().is_ok_and(|x| x < 1024)
                {
                    markup.push_str(&format!("line_height='{val} '"));
                }
            }
            "text_transform" | "transform" => match val {
                "l" | "lowercase" => markup.push_str("text_transform='lowercase' "),
                "u" | "uppercase" => markup.push_str("text_transform='uppercase' "),
                "c" | "capitalize" => markup.push_str("text_transform='capitalize' "),
                _ => markup.push_str("text_transform='none' "),
            },
            "segment" => match val {
                "w" | "word" => markup.push_str("segment='word' "),
                "s" | "sentence" => markup.push_str("segment='sentence' "),
                _ => {}
            },
            _ => defaults.apply(attr),
        };
    }
    (markup, defaults)
}

fn text_style(kind: &TestStyle, children: Children) -> Option<String> {
    let mut markup = String::new();
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let ElemKind::Label(Text::Style(style)) = derive_kind(child.tag_name().name()) {
                    if let Some(data) = text_style(&style, child.children()) {
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
        TestStyle::Bold => markup = format!("<b>{markup}</b>"),
        TestStyle::Italic => markup = format!("<i>{markup}</i>"),
        TestStyle::Underline => markup = format!("<u>{markup}</u>"),
        TestStyle::Strikethrough => markup = format!("<s>{markup}</s>"),
        TestStyle::Superscript => markup = format!("<sup>{markup}</sup>"),
        TestStyle::Subscript => markup = format!("<sub>{markup}</sub>"),
        TestStyle::Code => markup = format!("<tt>{markup}</tt>"),
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

#[derive(Debug, PartialEq)]
enum Text {
    Kind(TextKind),
    Style(TestStyle),
}

#[derive(Debug, PartialEq)]
enum TextKind {
    Header1,
    Header2,
    Header3,
    Header4,
    Header5,
    Header6,
    Normal,
}

#[derive(Debug, PartialEq)]
enum TestStyle {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Superscript,
    Subscript,
    Code,
}
/* #endregion Label */
/* #region Grid */
fn process_grid(children: Children, attributes: Attributes) -> Option<Widget> {
    let mut colhom = true;
    let mut rowhom = false;
    let mut defaults = WidgetDefaults::new();
    for attr in attributes {
        match attr.name() {
            "column_homogeneous" | "col" => match attr.value() {
                "n" | "f" | "no" | "false" => colhom = false,
                _ => colhom = true,
            },
            "row_homogeneous" | "row" => match attr.value() {
                "n" | "f" | "no" | "false" => rowhom = false,
                _ => rowhom = true,
            },
            _ => defaults.apply(attr),
        }
    }
    let grid = gtk::Grid::builder()
        .column_homogeneous(colhom)
        .row_homogeneous(rowhom)
        .halign(defaults.halign)
        .valign(defaults.valign)
        .hexpand(defaults.hexpand)
        .vexpand(defaults.vexpand)
        .build();
    for child in children {
        if derive_kind(child.tag_name().name()) != ElemKind::Grid(GridKind::GridItem) {
            println!("Expected GridItem, found: {child:?}");
            continue;
        }
        let (mut column, mut row, mut width, mut height) = (0i32, 0i32, 0i32, 0i32);
        for attr in child.attributes() {
            let val = attr.value();
            match attr.name() {
                "c" | "column" => {
                    if let Ok(val) = val.parse::<i32>() {
                        column = val;
                    }
                }
                "r" | "row" => {
                    if let Ok(val) = val.parse::<i32>() {
                        row = val;
                    }
                }
                "w" | "width" => {
                    if let Ok(val) = val.parse::<i32>() {
                        width = val;
                    }
                }
                "h" | "height" => {
                    if let Ok(val) = val.parse::<i32>() {
                        height = val;
                    }
                }
                _ => {}
            };
        }
        for baby in child.children() {
            if baby.is_text() && baby.text().is_some_and(|x| x.trim().is_empty()) {
                continue;
            } else if let Some(widget) = process_element(&baby) {
                grid.attach(&widget, column, row, width, height);
                break;
            }
        }
    }
    Some(grid.into())
}

#[derive(Debug, PartialEq)]
enum GridKind {
    Grid,
    GridItem,
}
/* #endregion Grid */
/* #region Container */
fn process_container(children: Children, attributes: Attributes) -> Option<Widget> {
    None
}
/* #endregion Container */

struct WidgetDefaults {
    halign: gtk::Align,
    valign: gtk::Align,
    hexpand: bool,
    vexpand: bool,
}

impl WidgetDefaults {
    pub fn new() -> Self {
        Self {
            halign: gtk::Align::Start,
            valign: gtk::Align::Start,
            hexpand: false,
            vexpand: false,
        }
    }
    fn apply(&mut self, attr: roxmltree::Attribute) {
        let val = attr.value();
        match attr.name() {
            "HALIGN" => match val {
                "fill" => self.halign = gtk::Align::Fill,
                "start" | "left" => self.halign = gtk::Align::Start,
                "end" | "right" => self.halign = gtk::Align::End,
                "center" | "middle" => self.halign = gtk::Align::Center,
                "baseline" => self.halign = gtk::Align::Baseline,
                _ => {}
            },
            "VALIGN" => match val {
                "fill" => self.valign = gtk::Align::Fill,
                "start" | "left" => self.valign = gtk::Align::Start,
                "end" | "right" => self.valign = gtk::Align::End,
                "center" | "middle" => self.valign = gtk::Align::Center,
                "baseline" => self.valign = gtk::Align::Baseline,
                _ => {}
            },
            "HEXPAND" => match val {
                "true" | "t" | "yes" | "y" => self.hexpand = true,
                _ => self.hexpand = false,
            },
            "VEXPAND" => match val {
                "true" | "t" | "yes" | "y" => self.vexpand = true,
                _ => self.vexpand = false,
            },
            _ => {}
        }
    }
}

#[derive(Debug, PartialEq)]
enum ElemKind {
    Label(Text),
    Grid(GridKind),
    Container,
    Fallback,
}
