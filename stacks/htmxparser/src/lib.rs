use gtk4::{Widget, prelude::*};
use roxmltree::{Attributes, Children, Document, Node, NodeType};

#[unsafe(no_mangle)]
pub fn stacks() -> String {
    "HTMLX".to_owned()
}

#[unsafe(no_mangle)]
pub fn get_elements(markup: String) -> gtk4::Box {
    let mut markup = markup;
    if !markup.starts_with("<xml") {
        markup = format!("<xml>{markup}</xml>");
    }
    let _ = gtk4::init();
    let webview = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    let tree = match Document::parse(&markup) {
        Ok(tree) => tree,
        Err(e) => {
            let label = gtk4::Label::builder()
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
        if let Some(element) = process_element(&element, &webview) {
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
        "p" | "_" => ElemKind::Label(Text::Kind(TextKind::Normal)),
        "i" => ElemKind::Label(Text::Style(TestStyle::Italic)),
        "b" => ElemKind::Label(Text::Style(TestStyle::Bold)),
        "u" => ElemKind::Label(Text::Style(TestStyle::Underline)),
        "s" => ElemKind::Label(Text::Style(TestStyle::Strikethrough)),
        "sup" => ElemKind::Label(Text::Style(TestStyle::Superscript)),
        "sub" => ElemKind::Label(Text::Style(TestStyle::Subscript)),
        "code" => ElemKind::Label(Text::Style(TestStyle::Code)),
        "grid" => ElemKind::Container(BoxKind::Grid),
        "griditem" | "gi" => ElemKind::Container(BoxKind::GridItem),
        "div" | "box" => ElemKind::Container(BoxKind::Normal),
        "button" => ElemKind::Button(ButtonKind::Normal),
        "toggle" | "tbutton" => ElemKind::Button(ButtonKind::Toggle),
        "checked" | "check" | "cbutton" | "radio" | "rbutton" => {
            ElemKind::Button(ButtonKind::Checked)
        }
        "canvas" | "draw" | "drawingarea" => ElemKind::Canvas(CanvasKind::DrawingArea),
        "gl" | "glarea" => ElemKind::Canvas(CanvasKind::GLArea),
        _ => ElemKind::Fallback,
    }
}

fn process_element(element: &Node, parent: &gtk4::Box) -> Option<Widget> {
    let kind = derive_kind(element.tag_name().name());
    match kind {
        ElemKind::Label(kind) => process_label(&kind, element.children(), element.attributes()),
        ElemKind::Container(BoxKind::Grid) => {
            process_grid(element.children(), element.attributes(), parent)
        }
        ElemKind::Container(BoxKind::GridItem) => None,
        ElemKind::Container(BoxKind::Normal) => {
            process_box(element.children(), element.attributes())
        }
        ElemKind::Button(kind) => match kind {
            ButtonKind::Normal => normal_button(element.children(), element.attributes(), parent),
            ButtonKind::Toggle => toggle_button(element.children(), element.attributes(), parent),
            ButtonKind::Checked => checked_button(element.children(), element.attributes(), parent),
        },
        ElemKind::Canvas(kind) => canvas_area(element.attributes(), kind),
        ElemKind::Fallback => element
            .text()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|text| {
                gtk4::Label::builder()
                    .halign(gtk4::Align::Start)
                    .valign(gtk4::Align::Start)
                    .label(text.trim())
                    .build()
                    .into()
            }),
    }
}

/* #region Labels */
fn process_label(kind: &Text, children: Children, attributes: Attributes) -> Option<Widget> {
    let (text, defaults) = process_text(kind, children, attributes)?;
    let label = gtk4::Label::builder().use_markup(true).build();
    label.set_label(&text);
    let label = label.into();
    defaults.apply(&label);
    Some(label)
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
                if let ElemKind::Label(text) = derive_kind(child.tag_name().name()) {
                    match text {
                        Text::Kind(kind) => {
                            if let Some((data, _)) = process_text(
                                &Text::Kind(kind),
                                child.children(),
                                child.attributes(),
                            ) {
                                markup.push_str(&data);
                            }
                        }
                        Text::Style(style) => {
                            if let Some(data) = text_style(&style, child.children()) {
                                markup.push_str(&data);
                            }
                        }
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
                if gtk4::pango::FontDescription::from_string(val).size() != 0 {
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
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("color='{val}' "))
                }
            }
            "background" | "bgcolor" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
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
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    markup.push_str(&format!("underline_color='{val}' "))
                }
            }
            "overline" => match val {
                "s" | "single" => markup.push_str("overline='single' "),
                _ => markup.push_str("overline='none' "),
            },
            "olc" | "overline_color" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
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
                if gtk4::gdk::RGBA::parse(val).is_ok() {
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
            _ => defaults.modify(attr),
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
/* #endregion Labels */
/* #region Containers */
fn process_box(children: Children, attributes: Attributes) -> Option<Widget> {
    let mut defaults = WidgetDefaults::new();
    let container = gtk4::Box::builder().build();
    for attr in attributes {
        defaults.modify(attr);
    }
    for child in children {
        if let Some(widget) = process_element(&child, &container) {
            container.append(&widget);
        }
    }
    let container = container.into();
    defaults.apply(&container);
    Some(container)
}

fn process_grid(children: Children, attributes: Attributes, parent: &gtk4::Box) -> Option<Widget> {
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
            _ => defaults.modify(attr),
        }
    }
    let grid = gtk4::Grid::builder()
        .column_homogeneous(colhom)
        .row_homogeneous(rowhom)
        .build();
    for child in children {
        if derive_kind(child.tag_name().name()) != ElemKind::Container(BoxKind::GridItem) {
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
            } else if let Some(widget) = process_element(&baby, parent) {
                grid.attach(&widget, column, row, width, height);
                break;
            }
        }
    }
    let grid = grid.into();
    defaults.apply(&grid);
    Some(grid)
}

#[derive(Debug, PartialEq)]
enum BoxKind {
    Grid,
    GridItem,
    Normal,
}

/* #endregion Containers */
/* #region Buttons */
fn normal_button(children: Children, attributes: Attributes, parent: &gtk4::Box) -> Option<Widget> {
    let mut defaults = WidgetDefaults::new();
    let button = gtk4::Button::builder().build();
    for attr in attributes {
        defaults.modify(attr);
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            button.set_child(Some(&widget));
            break;
        }
    }
    let button = button.into();
    defaults.apply(&button);
    Some(button)
}

fn toggle_button(children: Children, attributes: Attributes, parent: &gtk4::Box) -> Option<Widget> {
    let mut defaults = WidgetDefaults::new();
    let button = gtk4::ToggleButton::builder().build();
    for attr in attributes {
        match attr.name() {
            "checked" | "check" => match attr.value() {
                "no" | "n" | "false" | "f" => button.set_active(false),
                _ => button.set_active(true),
            },
            "group" => {
                let mut child = parent.first_child();
                while let Some(cur_child) = child {
                    if let Some(cur_child) = cur_child.downcast_ref::<gtk4::ToggleButton>() {
                        if cur_child.widget_name() == attr.value() {
                            button.set_group(Some(cur_child));
                        }
                    }
                    child = cur_child.next_sibling();
                }
            }
            _ => defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            button.set_child(Some(&widget));
            break;
        }
    }
    let button = button.into();
    defaults.apply(&button);
    Some(button)
}

fn checked_button(
    children: Children,
    attributes: Attributes,
    parent: &gtk4::Box,
) -> Option<Widget> {
    let mut defaults = WidgetDefaults::new();
    let button = gtk4::CheckButton::builder().build();
    for attr in attributes {
        match attr.name() {
            "checked" | "check" => match attr.value() {
                "no" | "n" | "false" | "f" => button.set_active(false),
                _ => button.set_active(true),
            },
            "group" => {
                let mut child = parent.first_child();
                while let Some(cur_child) = child {
                    if let Some(cur_child) = cur_child.downcast_ref::<gtk4::CheckButton>() {
                        if cur_child.widget_name() == attr.value() {
                            button.set_group(Some(cur_child));
                        }
                    }
                    child = cur_child.next_sibling();
                }
            }
            _ => defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            // POV when gtk4-rs forgot to implement ButtonExt for CheckButton, so you
            // have to implement `CheckButton::set_child(Option<&impl IsA<Widget>)` yourself:
            button.set_property("child", Some(&widget));
            break;
        }
    }
    let button = button.into();
    defaults.apply(&button);
    Some(button)
}

#[derive(Debug, PartialEq)]
enum ButtonKind {
    Normal,
    Toggle,
    Checked,
}
/* #endregion Buttons */
/* #region Canvases */
fn canvas_area(attributes: Attributes, kind: CanvasKind) -> Option<Widget> {
    let mut defaults = WidgetDefaults::new();
    let canvas: Widget = match kind {
        CanvasKind::GLArea => gtk4::GLArea::builder().build().into(),
        CanvasKind::DrawingArea => gtk4::DrawingArea::builder().build().into(),
    };
    for attr in attributes {
        defaults.modify(attr);
    }
    defaults.apply(&canvas);
    Some(canvas)
}

#[derive(Debug, PartialEq)]
enum CanvasKind {
    GLArea,
    DrawingArea,
}
/* #endregion Canvases */

#[derive(Debug)]
struct Margin {
    top: i32,
    bottom: i32,
    start: i32,
    end: i32,
}

impl Margin {
    pub fn new() -> Self {
        Self {
            top: 0,
            bottom: 0,
            start: 0,
            end: 0,
        }
    }
}
#[derive(Debug)]
struct WidgetDefaults {
    halign: gtk4::Align,
    valign: gtk4::Align,
    hexpand: bool,
    vexpand: bool,
    tooltip: Option<String>,
    opacity: f64,
    margin: Margin,
    name: String,
}

impl WidgetDefaults {
    pub fn new() -> Self {
        Self {
            halign: gtk4::Align::Start,
            valign: gtk4::Align::Start,
            hexpand: false,
            vexpand: false,
            tooltip: None,
            opacity: 1f64,
            margin: Margin::new(),
            name: String::new(),
        }
    }
    fn modify(&mut self, attr: roxmltree::Attribute) {
        let val = attr.value();
        match attr.name() {
            "_halign" => match val {
                "fill" => self.halign = gtk4::Align::Fill,
                "start" | "left" => self.halign = gtk4::Align::Start,
                "end" | "right" => self.halign = gtk4::Align::End,
                "center" | "middle" => self.halign = gtk4::Align::Center,
                "baseline" => self.halign = gtk4::Align::Baseline,
                _ => {}
            },
            "_valign" => match val {
                "fill" => self.valign = gtk4::Align::Fill,
                "start" | "left" => self.valign = gtk4::Align::Start,
                "end" | "right" => self.valign = gtk4::Align::End,
                "center" | "middle" => self.valign = gtk4::Align::Center,
                "baseline" => self.valign = gtk4::Align::Baseline,
                _ => {}
            },
            "_hexpand" => match val {
                "true" | "t" | "yes" | "y" => self.hexpand = true,
                _ => self.hexpand = false,
            },
            "_vexpand" => match val {
                "true" | "t" | "yes" | "y" => self.vexpand = true,
                _ => self.vexpand = false,
            },
            "_tooltip" => {
                let val = val.trim().to_owned();
                self.tooltip = if val.is_empty() { None } else { Some(val) }
            }
            "_opacity" => {
                if let Ok(val) = val.parse::<f64>() {
                    self.opacity = val;
                }
            }
            "_margin" => {
                let mut val = val.split(",");
                if let Some(Ok(top)) = val.nth(0).map(|x| x.parse::<i32>()) {
                    self.margin.top = top;
                }
                if let Some(Ok(bottom)) = val.nth(0).map(|x| x.parse::<i32>()) {
                    self.margin.bottom = bottom;
                }
                if let Some(Ok(start)) = val.nth(0).map(|x| x.parse::<i32>()) {
                    self.margin.start = start;
                }
                if let Some(Ok(end)) = val.nth(0).map(|x| x.parse::<i32>()) {
                    self.margin.end = end;
                }
            }
            "_name" => self.name = val.trim().to_string(),
            _ => {}
        }
    }
    fn apply(&self, widget: &Widget) {
        widget.set_hexpand(self.hexpand);
        widget.set_vexpand(self.vexpand);
        widget.set_halign(self.halign);
        widget.set_valign(self.valign);
        widget.set_tooltip_text(self.tooltip.as_deref());
        widget.set_opacity(self.opacity);
        widget.set_margin_top(self.margin.top);
        widget.set_margin_bottom(self.margin.bottom);
        widget.set_margin_start(self.margin.start);
        widget.set_margin_end(self.margin.end);
        widget.set_widget_name(&self.name);
    }
}

#[derive(Debug, PartialEq)]
enum ElemKind {
    Label(Text),
    Container(BoxKind),
    Button(ButtonKind),
    Canvas(CanvasKind),
    Fallback,
}
