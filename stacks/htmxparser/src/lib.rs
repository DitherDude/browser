use gtk4::{Widget, prelude::*};
use roxmltree::{Attributes, Children, Document, Node, NodeType};
/* #region Init */
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
    let mut data = BoxData::new();
    let mut defaults = WidgetDefaults::new();
    defaults.hexpand = true;
    defaults.vexpand = true;
    defaults.halign = gtk4::Align::Fill;
    defaults.valign = gtk4::Align::Fill;
    let tree = match Document::parse(&markup) {
        Ok(tree) => tree,
        Err(e) => {
            let webview = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .build();
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
        if let Some(element) = process_element(&element, &data) {
            data.children.push(element);
        } else {
            println!("Unrecognised element: {element:#?}");
        }
    }
    data.dirty_build(Some(&defaults))
}

fn derive_kind(name: &str) -> ElemKind {
    match name {
        "h1" => ElemKind::Label(Text::Kind(TextKind::Header1)),
        "h2" => ElemKind::Label(Text::Kind(TextKind::Header2)),
        "h3" => ElemKind::Label(Text::Kind(TextKind::Header3)),
        "h4" => ElemKind::Label(Text::Kind(TextKind::Header4)),
        "h5" => ElemKind::Label(Text::Kind(TextKind::Header5)),
        "h6" => ElemKind::Label(Text::Kind(TextKind::Header6)),
        "p" | "_" | "a" => ElemKind::Label(Text::Kind(TextKind::Normal)),
        "i" => ElemKind::Label(Text::Style(TextStyle::Italic)),
        "b" => ElemKind::Label(Text::Style(TextStyle::Bold)),
        "u" => ElemKind::Label(Text::Style(TextStyle::Underline)),
        "s" => ElemKind::Label(Text::Style(TextStyle::Strikethrough)),
        "sup" => ElemKind::Label(Text::Style(TextStyle::Superscript)),
        "sub" => ElemKind::Label(Text::Style(TextStyle::Subscript)),
        "code" => ElemKind::Label(Text::Style(TextStyle::Code)),
        "grid" => ElemKind::Container(ContainerKind::Grid),
        "griditem" | "gi" => ElemKind::Container(ContainerKind::GridItem),
        "div" | "box" => ElemKind::Container(ContainerKind::Normal),
        "button" | "btn" => ElemKind::Button(ButtonKind::Normal),
        "toggle" | "tbtn" => ElemKind::Button(ButtonKind::Toggle),
        "checked" | "check" | "cbtn" | "radio" | "rbtn" => ElemKind::Button(ButtonKind::Checked),
        "canvas" | "draw" | "drawingarea" => ElemKind::Canvas(CanvasKind::DrawingArea),
        "gl" | "glarea" => ElemKind::Canvas(CanvasKind::GLArea),
        "clone" | "cloned" => ElemKind::Cloned,
        "spinner" | "spin" => ElemKind::Loader(LoaderKind::Spinner),
        "levelbar" | "lb" => ElemKind::Loader(LoaderKind::LevelBar),
        "progressbar" | "pb" => ElemKind::Loader(LoaderKind::ProgressBar),
        "textview" | "multiline" => ElemKind::Input(InputKind::TextView),
        "entry" | "input" => ElemKind::Input(InputKind::Entry),
        "search" | "searchentry" => ElemKind::Input(InputKind::Search),
        "password" | "pass" => ElemKind::Input(InputKind::Password),
        "number" | "valuepicker" => ElemKind::Input(InputKind::Spin),
        "editable" => ElemKind::Input(InputKind::Editable),
        _ => ElemKind::Fallback,
    }
}

fn process_element(elem: &Node, parent: &BoxData) -> Option<WidgetData> {
    let kind = derive_kind(elem.tag_name().name());
    let mut defaults = WidgetDefaults::new();
    let data = match kind {
        ElemKind::Label(kind) => process_label(&kind, elem.children(), elem.attributes(), parent),
        ElemKind::Container(kind) => match kind {
            ContainerKind::Normal => process_box(elem.children()),
            ContainerKind::Grid => process_grid(elem.children(), elem.attributes()),
            ContainerKind::GridItem => None,
        },
        ElemKind::Button(kind) => match kind {
            ButtonKind::Normal => normal_button(elem.children(), parent),
            ButtonKind::Toggle => toggle_button(elem.children(), elem.attributes(), parent),
            ButtonKind::Checked => check_button(elem.children(), elem.attributes(), parent),
        },
        ElemKind::Canvas(canvas) => match canvas {
            CanvasKind::DrawingArea => drawing_area(),
            CanvasKind::GLArea => gl_area(),
        },
        ElemKind::Loader(loader) => match loader {
            LoaderKind::Spinner => spinner(elem.attributes()),
            LoaderKind::LevelBar => level_bar(elem.attributes()),
            LoaderKind::ProgressBar => progress_bar(elem.children(), elem.attributes(), parent),
        },
        ElemKind::Input(input) => match input {
            InputKind::TextView => process_textview(elem.children(), elem.attributes(), parent),
            InputKind::Entry => process_entry(elem.attributes()),
            InputKind::Search => search_entry(elem.attributes()),
            InputKind::Password => password_entry(elem.attributes()),
            InputKind::Spin => spin_button(elem.attributes()),
            InputKind::Editable => editable_label(elem.children(), elem.attributes(), parent),
        },
        ElemKind::Cloned => {
            if !elem.has_children() {
                if let Some(widget) = process_cloned(elem.attributes(), parent) {
                    defaults = widget.defaults;
                    Some(widget.data)
                } else {
                    None
                }
            } else {
                None
            }
        }
        ElemKind::Fallback => elem.text().filter(|x| !x.trim().is_empty()).map(|text| {
            let mut data = LabelData::new();
            data.text = text.to_string();
            DataEnum::Label(Box::new(data))
        }),
    };
    if let Some(data) = data {
        for attr in elem.attributes() {
            defaults.modify(attr, parent);
        }
        Some(WidgetData { defaults, data })
    } else {
        None
    }
}
/* #endregion Init */
/* #region Labels */
fn process_label(
    kind: &Text,
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<DataEnum> {
    process_text(kind, children, attributes, parent).map(|data| DataEnum::Label(Box::new(data)))
}

fn process_text(
    kind: &Text,
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<LabelData> {
    match kind {
        Text::Kind(kind) => text_kind(kind, children, attributes, None, parent),
        Text::Style(style) => {
            let mut data = LabelData::new();
            data.text = raw_text_style(style, children)?;
            Some(data)
        }
    }
}

fn text_kind(
    kind: &TextKind,
    children: Children,
    attributes: Attributes,
    dad: Option<&LabelData>,
    parent: &BoxData,
) -> Option<LabelData> {
    let mut data = text_attributes(attributes, dad);
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                let name = child.tag_name().name();
                if let Some(label) = if let ElemKind::Cloned = derive_kind(name) {
                    if let Some(WidgetData {
                        defaults: _,
                        data: DataEnum::Clone(clone),
                    }) = process_cloned(child.attributes(), parent)
                    {
                        if let WidgetData {
                            data: DataEnum::Label(label),
                            ..
                        } = *clone
                        {
                            Some(*label)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else if let ElemKind::Label(label) = derive_kind(name) {
                    match label {
                        Text::Kind(kind) => text_kind(
                            &kind,
                            child.children(),
                            child.attributes(),
                            Some(&data),
                            parent,
                        ),
                        Text::Style(style) => {
                            if let Some(text) = raw_text_style(&style, child.children()) {
                                if data.text.is_empty() {
                                    data.text = text;
                                    None
                                } else {
                                    let mut raw_data = data.clone();
                                    raw_data.children = Vec::new();
                                    raw_data.text = text;
                                    Some(raw_data)
                                }
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                } {
                    data.children.push(label);
                }
            }
            NodeType::Text => {
                if let Some(text) = child.text() {
                    if data.text.is_empty() {
                        data.text = text.to_string();
                    } else {
                        let mut raw_data = data.clone();
                        raw_data.children = Vec::new();
                        raw_data.text = text.to_string();
                        data.children.push(raw_data);
                    }
                }
            }
            _ => {}
        }
    }
    match kind {
        TextKind::Header1 => data.size = Some("xx-large".to_string()),
        TextKind::Header2 => data.size = Some("x-large".to_string()),
        TextKind::Header3 => data.size = Some("large".to_string()),
        TextKind::Header4 => data.size = Some("medium".to_string()),
        TextKind::Header5 => data.size = Some("small".to_string()),
        TextKind::Header6 => data.size = Some("x-small".to_string()),
        TextKind::Normal => {}
    }
    Some(data)
}

fn text_attributes(attributes: Attributes, dad: Option<&LabelData>) -> LabelData {
    let mut data = if let Some(dad) = dad {
        let mut data = dad.clone();
        data.text = String::new();
        data.children = Vec::new();
        data
    } else {
        LabelData::new()
    };
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "href" | "link" => data.link = Some(val.to_string()),
            "font" => data.font = Some(val.to_string()),
            "ff" | "font_family" | "face" => data.face = Some(val.to_string()),
            "size" => {
                let val = gtk4::pango::FontDescription::from_string(val).size();
                if val != 0 {
                    data.size = Some(val.to_string());
                }
            }
            "style" => match val {
                "o" | "oblique" => data.style = Some(l_attr::Style::Oblique),
                "i" | "italic" => data.style = Some(l_attr::Style::Italic),
                _ => data.style = Some(l_attr::Style::Normal),
            },
            "weight" => match val {
                "ul" | "ultralight" => data.weight = Some(l_attr::Weight::UltraLight),
                "l" | "light" => data.weight = Some(l_attr::Weight::Light),
                "b" | "bold" => data.weight = Some(l_attr::Weight::Bold),
                "ub" | "ultrabold" => data.weight = Some(l_attr::Weight::UltraBold),
                "h" | "heavy" => data.weight = Some(l_attr::Weight::Heavy),
                _ => data.weight = Some(l_attr::Weight::Normal),
            },
            "variant" => match val {
                "sc" | "small-caps" | "small_caps" | "smallcaps" => {
                    data.variant = Some(l_attr::Variant::SmallCaps)
                }
                "asc" | "all-small-caps" | "all_small_caps" | "allsmallcaps" => {
                    data.variant = Some(l_attr::Variant::AllSmallCaps)
                }
                "pc" | "petite-caps" | "petite_caps" | "petitecaps" => {
                    data.variant = Some(l_attr::Variant::PetiteCaps)
                }
                "apc" | "all-petite-caps" | "all_petite_caps" | "allpetitecaps" => {
                    data.variant = Some(l_attr::Variant::AllPetiteCaps)
                }
                "uc" | "unicase" => data.variant = Some(l_attr::Variant::Unicase),
                "tc" | "title-caps" | "title_caps" | "titlecaps" => {
                    data.variant = Some(l_attr::Variant::TitleCaps)
                }
                _ => data.variant = Some(l_attr::Variant::Normal),
            },
            "font_features" | "features" => data.features = Some(val.to_string()),
            "foreground" | "fgcolor" | "color" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    data.fcolor = Some(val.to_string());
                }
            }
            "background" | "bgcolor" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    data.bcolor = Some(val.to_string());
                }
            }
            "alpha" | "fgalpha" => {
                if let Ok(val) = val.parse::<u16>() {
                    data.falpha = Some(format!("{}", val as u32 + 1));
                } else if val
                    .strip_suffix("%")
                    .filter(|x| x.parse::<u8>().is_ok_and(|x| x <= 100))
                    .is_some()
                {
                    data.falpha = Some(val.to_string());
                }
            }
            "background_alpha" | "bgalpha" => {
                if let Ok(val) = val.parse::<u16>() {
                    data.balpha = Some(format!("{}", val as u32 + 1));
                } else if val
                    .strip_suffix("%")
                    .filter(|x| x.parse::<u8>().is_ok_and(|x| x <= 100))
                    .is_some()
                {
                    data.balpha = Some(val.to_string());
                }
            }
            "underline" => match val {
                "s" | "single" => data.underline = Some(l_attr::UnderLine::Single),
                "d" | "double" => data.underline = Some(l_attr::UnderLine::Double),
                "l" | "low" => data.underline = Some(l_attr::UnderLine::Low),
                "e" | "error" => data.underline = Some(l_attr::UnderLine::Error),
                _ => data.underline = Some(l_attr::UnderLine::None),
            },
            "ulc" | "underline_color" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    data.ulc = Some(val.to_string());
                }
            }
            "overline" => match val {
                "n" | "none" | "no" | "f" | "false" => data.overline = Some(false),
                _ => data.overline = Some(true),
            },
            "olc" | "overline_color" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    data.olc = Some(val.to_string());
                }
            }
            "rise" => {
                if val.strip_suffix("pt").unwrap_or(val).parse::<i32>().is_ok() {
                    data.rise = Some(val.to_string());
                }
            }
            "baseline_shift" | "shift" => {
                if val.strip_suffix("pt").unwrap_or(val).parse::<i32>().is_ok() {
                    data.shift = Some(val.to_string());
                }
            }
            "font_scale" | "scale" => match val {
                "sup" | "superscript" => data.scale = Some(l_attr::Scale::Superscript),
                "sub" | "subscript" => data.scale = Some(l_attr::Scale::Subscript),
                "sc" | "small-caps" | "small_caps" | "smallcaps" => {
                    data.scale = Some(l_attr::Scale::SmallCaps)
                }
                _ => {}
            },
            "s" | "strikethrough" => match val.to_lowercase().as_str() {
                "n" | "no" | "f" | "false" => data.strikethrough = Some(false),
                _ => data.strikethrough = Some(true),
            },
            "strikethrough_color" | "scolor" => {
                if gtk4::gdk::RGBA::parse(val).is_ok() {
                    data.scolor = Some(val.to_string());
                }
            }
            "fallback" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => data.fallback = Some(false),
                _ => data.fallback = Some(true),
            },
            "lang" => data.lang = Some(val.to_string()),
            "gravity" => match val {
                "south" | "bottom" => data.gravity = Some(l_attr::Gravity::South),
                "east" | "right" => data.gravity = Some(l_attr::Gravity::East),
                "north" | "top" => data.gravity = Some(l_attr::Gravity::North),
                "west" | "left" => data.gravity = Some(l_attr::Gravity::West),
                _ => data.gravity = Some(l_attr::Gravity::Auto),
            },
            "gravity_hint" | "hint" => match val {
                "s" | "strong" => data.hint = Some(l_attr::GravityHint::Strong),
                "l" | "line" => data.hint = Some(l_attr::GravityHint::Line),
                _ => data.hint = Some(l_attr::GravityHint::Natural),
            },
            "show" => {
                if val
                    .split('|')
                    .all(|x| ["spaces", "line-breaks", "ignorables"].contains(&x))
                    || val == "none"
                {
                    data.show = Some(val.to_string());
                }
            }
            "insert_hyphens" | "hyphens" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => data.hyphens = Some(false),
                _ => data.hyphens = Some(true),
            },
            "allow_breaks" | "breaks" => match val.to_lowercase().as_str() {
                "n" | "f" | "no" | "false" => data.breaks = Some(false),
                _ => data.breaks = Some(true),
            },
            "line_height" | "height" => {
                if val
                    .strip_suffix("pt")
                    .is_some_and(|x| x.parse::<f64>().is_ok_and(|x| x >= 0f64))
                    || val.parse::<u16>().is_ok_and(|x| x < 1024)
                {
                    data.height = Some(val.to_string());
                }
            }
            "text_transform" | "transform" => match val {
                "l" | "lowercase" => data.transform = Some(l_attr::Transform::Lowercase),
                "u" | "uppercase" => data.transform = Some(l_attr::Transform::Uppercase),
                "c" | "capitalize" => data.transform = Some(l_attr::Transform::Capitalize),
                _ => data.transform = Some(l_attr::Transform::None),
            },
            "segment" => match val {
                "w" | "word" => data.segment = Some(l_attr::Segment::Word),
                "s" | "sentence" => data.segment = Some(l_attr::Segment::Sentence),
                _ => {}
            },
            _ => {}
        };
    }
    data
}

mod l_attr {
    #[derive(Debug, PartialEq, Clone)]
    pub enum Style {
        Italic,
        Oblique,
        Normal,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Weight {
        UltraLight,
        Light,
        Normal,
        Bold,
        UltraBold,
        Heavy,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Variant {
        SmallCaps,
        AllSmallCaps,
        PetiteCaps,
        AllPetiteCaps,
        Unicase,
        TitleCaps,
        Normal,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum UnderLine {
        Single,
        Double,
        Low,
        Error,
        None,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Scale {
        Superscript,
        Subscript,
        SmallCaps,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Gravity {
        South,
        East,
        North,
        West,
        Auto,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum GravityHint {
        Strong,
        Line,
        Natural,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Transform {
        Lowercase,
        Uppercase,
        Capitalize,
        None,
    }
    #[derive(Debug, PartialEq, Clone)]
    pub enum Segment {
        Word,
        Sentence,
    }
}
#[derive(Debug, PartialEq, Clone)]
struct LabelData {
    text: String,

    link: Option<String>,
    font: Option<String>,
    face: Option<String>,
    size: Option<String>,
    style: Option<l_attr::Style>,
    weight: Option<l_attr::Weight>,
    variant: Option<l_attr::Variant>,
    features: Option<String>,
    fcolor: Option<String>,
    bcolor: Option<String>,
    falpha: Option<String>,
    balpha: Option<String>,
    underline: Option<l_attr::UnderLine>,
    ulc: Option<String>,
    overline: Option<bool>,
    olc: Option<String>,
    rise: Option<String>,
    shift: Option<String>,
    scale: Option<l_attr::Scale>,
    strikethrough: Option<bool>,
    scolor: Option<String>,
    fallback: Option<bool>,
    lang: Option<String>,
    gravity: Option<l_attr::Gravity>,
    hint: Option<l_attr::GravityHint>,
    show: Option<String>,
    hyphens: Option<bool>,
    breaks: Option<bool>,
    height: Option<String>,
    transform: Option<l_attr::Transform>,
    segment: Option<l_attr::Segment>,
    children: Vec<LabelData>,
}
impl LabelData {
    pub fn new() -> Self {
        LabelData {
            text: String::new(),
            link: None,
            font: None,
            face: None,
            size: None,
            style: None,
            weight: None,
            variant: None,
            features: None,
            fcolor: None,
            bcolor: None,
            falpha: None,
            balpha: None,
            underline: None,
            ulc: None,
            overline: None,
            olc: None,
            rise: None,
            shift: None,
            scale: None,
            strikethrough: None,
            scolor: None,
            fallback: None,
            lang: None,
            gravity: None,
            hint: None,
            show: None,
            hyphens: None,
            breaks: None,
            height: None,
            transform: None,
            segment: None,
            children: Vec::new(),
        }
    }
    pub fn compile(&self) -> gtk4::glib::GString {
        let mut a = String::new();
        if let Some(font) = &self.font {
            a.push_str(&format!("font='{font}' "));
        }
        if let Some(face) = &self.face {
            a.push_str(&format!("face='{face}' "))
        }
        if let Some(size) = &self.size {
            a.push_str(&format!("size='{size}' "))
        }
        if let Some(style) = &self.style {
            a.push_str(&format!(
                "style='{}' ",
                match *style {
                    l_attr::Style::Oblique => "oblique",
                    l_attr::Style::Italic => "italic",
                    l_attr::Style::Normal => "normal",
                }
            ));
        }
        if let Some(weight) = &self.weight {
            a.push_str(&format!(
                "weight='{}' ",
                match *weight {
                    l_attr::Weight::UltraLight => "ultralight",
                    l_attr::Weight::Light => "light",
                    l_attr::Weight::Normal => "normal",
                    l_attr::Weight::Bold => "bold",
                    l_attr::Weight::UltraBold => "ultrabold",
                    l_attr::Weight::Heavy => "heavy",
                }
            ));
        }
        if let Some(variant) = &self.variant {
            a.push_str(&format!(
                "variant='{}' ",
                match *variant {
                    l_attr::Variant::SmallCaps => "small-caps",
                    l_attr::Variant::AllSmallCaps => "all-small-caps",
                    l_attr::Variant::PetiteCaps => "petite-caps",
                    l_attr::Variant::AllPetiteCaps => "all-petite-caps",
                    l_attr::Variant::Unicase => "unicase",
                    l_attr::Variant::TitleCaps => "title-caps",
                    l_attr::Variant::Normal => "normal",
                }
            ));
        }
        if let Some(features) = &self.features {
            a.push_str(&format!("font_features='{features}' "));
        }
        if let Some(color) = &self.fcolor {
            a.push_str(&format!("color='{color}' "));
        }
        if let Some(color) = &self.bcolor {
            a.push_str(&format!("bgcolor='{color}' "));
        }
        if let Some(alpha) = &self.falpha {
            a.push_str(&format!("alpha='{alpha}' "));
        }
        if let Some(alpha) = &self.balpha {
            a.push_str(&format!("bgalpha='{alpha}' "));
        }
        if let Some(underline) = &self.underline {
            a.push_str(&format!(
                "underline='{}' ",
                match *underline {
                    l_attr::UnderLine::Single => "single",
                    l_attr::UnderLine::Double => "double",
                    l_attr::UnderLine::Low => "low",
                    l_attr::UnderLine::Error => "error",
                    l_attr::UnderLine::None => "none",
                }
            ));
        }
        if let Some(color) = &self.ulc {
            a.push_str(&format!("underline_color='{color}' "));
        }
        if let Some(overline) = &self.overline {
            if *overline {
                a.push_str("overline='single' ");
            } else {
                a.push_str("overline='none' ");
            }
        }
        if let Some(color) = &self.olc {
            a.push_str(&format!("overline_color='{color}' "));
        }
        if let Some(rise) = &self.rise {
            a.push_str(&format!("rise='{rise}' "));
        }
        if let Some(shift) = &self.shift {
            a.push_str(&format!("baseline_shift='{shift}' "));
        }
        if let Some(scale) = &self.scale {
            a.push_str(&format!(
                "font_scale='{}' ",
                match *scale {
                    l_attr::Scale::Superscript => "superscript",
                    l_attr::Scale::Subscript => "subscript",
                    l_attr::Scale::SmallCaps => "small-caps",
                }
            ));
        }
        if let Some(st) = &self.strikethrough {
            if *st {
                a.push_str("strikethrough='true' ");
            } else {
                a.push_str("strikethrough='false' ");
            }
        }
        if let Some(color) = &self.scolor {
            a.push_str(&format!("strikethrough_color='{color}' "));
        }
        if let Some(fb) = &self.fallback {
            if *fb {
                a.push_str("fallback='true' ");
            } else {
                a.push_str("fallback='false' ");
            }
        }
        if let Some(lang) = &self.lang {
            a.push_str(&format!("lang='{lang}' "));
        }
        if let Some(gravity) = &self.gravity {
            a.push_str(&format!(
                "gravity='{}' ",
                match gravity {
                    l_attr::Gravity::South => "south",
                    l_attr::Gravity::East => "east",
                    l_attr::Gravity::North => "north",
                    l_attr::Gravity::West => "west",
                    l_attr::Gravity::Auto => "auto",
                }
            ));
        }
        if let Some(hint) = &self.hint {
            a.push_str(&format!(
                "gravity_hint='{}' ",
                match hint {
                    l_attr::GravityHint::Strong => "strong",
                    l_attr::GravityHint::Line => "line",
                    l_attr::GravityHint::Natural => "natural",
                }
            ));
        }
        if let Some(show) = &self.show {
            a.push_str(&format!("show='{show}' "));
        }
        if let Some(hyphens) = &self.hyphens {
            if *hyphens {
                a.push_str("insert_hyphens='true' ");
            } else {
                a.push_str("insert_hyphens='false' ");
            }
        }
        if let Some(breaks) = &self.breaks {
            if *breaks {
                a.push_str("allow_breaks='true' ");
            } else {
                a.push_str("allow_breaks='false' ");
            }
        }
        if let Some(height) = &self.height {
            a.push_str(&format!("line_height='{height}' "));
        }
        if let Some(transform) = &self.transform {
            a.push_str(&format!(
                "text_transform='{}' ",
                match transform {
                    l_attr::Transform::Lowercase => "lowercase",
                    l_attr::Transform::Uppercase => "uppercase",
                    l_attr::Transform::Capitalize => "capitalize",
                    l_attr::Transform::None => "none",
                }
            ));
        }
        if let Some(segment) = &self.segment {
            a.push_str(&format!(
                "segment='{}' ",
                match segment {
                    l_attr::Segment::Word => "word",
                    l_attr::Segment::Sentence => "sentence",
                }
            ));
        }
        let mut text = node_escape(&self.text);
        if let Some(link) = &self.link {
            text = format!("<a href='{link}'>{text}</a>");
        }
        format!("<span {}>{}{}</span>", a.trim(), text, {
            let mut nested = String::new();
            for child in &self.children {
                nested.push_str(&child.compile());
            }
            nested
        })
        .into()
    }
    pub fn concat(&self) -> String {
        let mut text = self.text.to_string();
        self.children.iter().for_each(|x| text.push_str(&x.text));
        text
    }
}

impl DataTrait for LabelData {
    fn build(&self, _: &gtk4::Box) -> Widget {
        let markup = self.compile();
        let label = gtk4::Label::builder()
            .use_markup(true)
            .label(markup)
            .build();
        label.into()
    }
}

fn raw_text_style(kind: &TextStyle, children: Children) -> Option<String> {
    let mut markup = String::new();
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let ElemKind::Label(Text::Style(style)) = derive_kind(child.tag_name().name()) {
                    if let Some(data) = raw_text_style(&style, child.children()) {
                        markup.push_str(&data);
                    }
                }
            }
            NodeType::Text => {
                if let Some(text) = child.text() {
                    markup.push_str(text);
                }
            }
            _ => {}
        }
    }
    match kind {
        TextStyle::Bold => markup = format!("<b>{markup}</b>"),
        TextStyle::Italic => markup = format!("<i>{markup}</i>"),
        TextStyle::Underline => markup = format!("<u>{markup}</u>"),
        TextStyle::Strikethrough => markup = format!("<s>{markup}</s>"),
        TextStyle::Superscript => markup = format!("<sup>{markup}</sup>"),
        TextStyle::Subscript => markup = format!("<sub>{markup}</sub>"),
        TextStyle::Code => markup = format!("<tt>{markup}</tt>"),
    }
    Some(markup)
}

fn node_escape(raw: &str) -> String {
    raw.replace("&", "&amp;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
}
fn attr_escape(raw: &str) -> String {
    raw.replace("&", "&amp;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
}

#[derive(Debug, PartialEq, Clone)]
enum Text {
    Kind(TextKind),
    Style(TextStyle),
}

#[derive(Debug, PartialEq, Clone)]
enum TextKind {
    Header1,
    Header2,
    Header3,
    Header4,
    Header5,
    Header6,
    Normal,
}

#[derive(Debug, PartialEq, Clone)]
enum TextStyle {
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
fn process_box(children: Children) -> Option<DataEnum> {
    let mut data = BoxData::new();
    for child in children {
        if let Some(child) = process_element(&child, &data) {
            data.children.push(child);
        }
    }
    Some(DataEnum::Container(ContainerData::Box(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct BoxData {
    orientation: gtk4::Orientation,
    children: Vec<WidgetData>,
}

impl BoxData {
    pub fn new() -> Self {
        BoxData {
            orientation: gtk4::Orientation::Vertical,
            children: Vec::new(),
        }
    }
    pub fn dirty_build(&self, defaults: Option<&WidgetDefaults>) -> gtk4::Box {
        let widget = gtk4::Box::builder().orientation(self.orientation).build();
        for child in &self.children {
            if !child.get_shadow() {
                widget.append(&child.build(&widget));
            }
        }
        if let Some(defaults) = defaults {
            defaults.apply(&widget);
        }
        widget
    }
}

fn process_grid(children: Children, attributes: Attributes) -> Option<DataEnum> {
    let mut data = GridData::new();
    for attr in attributes {
        match attr.name() {
            "column_homogeneous" | "col" => match attr.value() {
                "n" | "f" | "no" | "false" => data.col_hom = false,
                _ => data.col_hom = true,
            },
            "row_homogeneous" | "row" => match attr.value() {
                "n" | "f" | "no" | "false" => data.row_hom = false,
                _ => data.row_hom = true,
            },
            _ => {}
        }
    }
    for child in children {
        if derive_kind(child.tag_name().name()) != ElemKind::Container(ContainerKind::GridItem) {
            if !(child.is_text() && child.text().is_some_and(|x| x.trim().is_empty())) {
                println!("Expected GridItem, found: {child:?}");
            }
            continue;
        }
        let mut gridchild = GridChild {
            defaults: WidgetDefaults::new(),
            parent: BoxData::new(),
            column: 0i32,
            row: 0i32,
            width: 1i32,
            height: 1i32,
        };
        for attr in child.attributes() {
            let val = attr.value();
            match attr.name() {
                "c" | "column" => {
                    if let Ok(val) = val.parse::<i32>() {
                        gridchild.column = val;
                    }
                }
                "r" | "row" => {
                    if let Ok(val) = val.parse::<i32>() {
                        gridchild.row = val;
                    }
                }
                "w" | "width" => {
                    if let Ok(Some(val)) = val
                        .parse::<i32>()
                        .map(|x| if x > 0 { Some(x) } else { None })
                    {
                        gridchild.width = val;
                    }
                }
                "h" | "height" => {
                    if let Ok(Some(val)) = val
                        .parse::<i32>()
                        .map(|x| if x > 0 { Some(x) } else { None })
                    {
                        gridchild.height = val;
                    }
                }
                _ => gridchild.defaults.modify(attr, &gridchild.parent),
            };
        }
        for child in child.children() {
            if child.is_text() && child.text().is_some_and(|x| x.trim().is_empty()) {
                continue;
            } else if let Some(widget) = process_element(&child, &gridchild.parent) {
                gridchild.parent.children.push(widget);
            }
        }
        data.children.push(gridchild);
    }
    Some(DataEnum::Container(ContainerData::Grid(data)))
}

#[derive(Debug, PartialEq, Clone)]
enum ContainerKind {
    Grid,
    GridItem,
    Normal,
}

#[derive(Debug, PartialEq, Clone)]
struct GridData {
    col_hom: bool,
    row_hom: bool,
    children: Vec<GridChild>,
}
#[derive(Debug, PartialEq, Clone)]
struct GridChild {
    defaults: WidgetDefaults,
    parent: BoxData,
    column: i32,
    row: i32,
    width: i32,
    height: i32,
}

impl GridData {
    pub fn new() -> Self {
        GridData {
            col_hom: true,
            row_hom: true,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ContainerData {
    Box(BoxData),
    Grid(GridData),
}

impl DataTrait for ContainerData {
    fn build(&self, _: &gtk4::Box) -> Widget {
        match &self {
            ContainerData::Box(container) => container.dirty_build(None).into(),
            ContainerData::Grid(container) => {
                let widget = gtk4::Grid::builder()
                    .column_homogeneous(container.col_hom)
                    .row_homogeneous(container.row_hom)
                    .build();
                for child in &container.children {
                    widget.attach(
                        &child.parent.dirty_build(Some(&child.defaults)),
                        child.column,
                        child.row,
                        child.width,
                        child.height,
                    );
                }
                widget.into()
            }
        }
    }
}

/* #endregion Containers */
/* #region Buttons */
fn normal_button(children: Children, parent: &BoxData) -> Option<DataEnum> {
    let mut data = NormalButtonData::new();
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(DataEnum::Button(ButtonData::Normal(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct NormalButtonData {
    child: Option<Box<WidgetData>>,
}

impl NormalButtonData {
    pub fn new() -> Self {
        NormalButtonData { child: None }
    }
}

fn toggle_button(children: Children, attributes: Attributes, parent: &BoxData) -> Option<DataEnum> {
    let mut data = ToggleButtonData::new();
    for attr in attributes {
        match attr.name() {
            "checked" | "check" => match attr.value() {
                "no" | "n" | "false" | "f" => data.checked = false,
                _ => data.checked = true,
            },
            "group" => {
                data.group = Some(attr.value().to_string());
            }
            _ => {}
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(DataEnum::Button(ButtonData::Toggle(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct ToggleButtonData {
    checked: bool,
    group: Option<String>,
    child: Option<Box<WidgetData>>,
}

impl ToggleButtonData {
    pub fn new() -> Self {
        ToggleButtonData {
            checked: false,
            group: None,
            child: None,
        }
    }
}

fn check_button(children: Children, attributes: Attributes, parent: &BoxData) -> Option<DataEnum> {
    let mut data = CheckButtonData::new();
    for attr in attributes {
        match attr.name() {
            "checked" | "check" => match attr.value() {
                "no" | "n" | "false" | "f" => data.checked = false,
                _ => data.checked = true,
            },
            "group" => {
                data.group = Some(attr.value().to_string());
            }
            _ => {}
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(DataEnum::Button(ButtonData::Checked(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct CheckButtonData {
    checked: bool,
    group: Option<String>,
    child: Option<Box<WidgetData>>,
}

impl CheckButtonData {
    pub fn new() -> Self {
        CheckButtonData {
            checked: false,
            group: None,
            child: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ButtonKind {
    Normal,
    Toggle,
    Checked,
}

#[derive(Debug, PartialEq, Clone)]
enum ButtonData {
    Normal(NormalButtonData),
    Toggle(ToggleButtonData),
    Checked(CheckButtonData),
}

impl DataTrait for ButtonData {
    fn build(&self, parent: &gtk4::Box) -> Widget {
        match &self {
            ButtonData::Normal(button) => {
                let widget = gtk4::Button::builder().build();
                if let Some(child) = &button.child {
                    if !child.get_shadow() {
                        let child = child.build(parent);
                        widget.set_child(Some(&child));
                    }
                }
                widget.into()
            }
            ButtonData::Toggle(button) => {
                let widget = gtk4::ToggleButton::builder().active(button.checked).build();
                if let Some(group) = &button.group {
                    let mut child = parent.first_child();
                    while let Some(cur_child) = child {
                        if let Some(cur_child) = cur_child.downcast_ref::<gtk4::ToggleButton>() {
                            if cur_child.widget_name() == *group {
                                widget.set_group(Some(cur_child));
                            }
                        }
                        child = cur_child.next_sibling();
                    }
                }
                if let Some(child) = &button.child {
                    if !child.get_shadow() {
                        let child = child.build(parent);
                        widget.set_child(Some(&child));
                    }
                }
                widget.into()
            }
            ButtonData::Checked(button) => {
                let widget = gtk4::CheckButton::builder().build();
                widget.set_active(button.checked);
                if let Some(group) = &button.group {
                    let mut child = parent.first_child();
                    while let Some(cur_child) = child {
                        if let Some(cur_child) = cur_child.downcast_ref::<gtk4::CheckButton>() {
                            if cur_child.widget_name() == *group {
                                widget.set_group(Some(cur_child));
                            }
                        }
                        child = cur_child.next_sibling();
                    }
                }
                if let Some(child) = &button.child {
                    if !child.get_shadow() {
                        let child = child.build(parent);
                        // POV when gtk4-rs forgot to implement ButtonExt for CheckButton, so you
                        // have to implement `CheckButton::set_child(Option<&impl IsA<Widget>)` yourself:
                        widget.set_property("child", Some(&child));
                    }
                }
                widget.into()
            }
        }
    }
}
/* #endregion Buttons */
/* #region Canvases */
fn gl_area() -> Option<DataEnum> {
    let data = GLAreaData::new();
    Some(DataEnum::Canvas(CanvasData::GLArea(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct GLAreaData {}

impl GLAreaData {
    pub fn new() -> Self {
        GLAreaData {}
    }
}

fn drawing_area() -> Option<DataEnum> {
    let data = DrawingAreaData::new();
    Some(DataEnum::Canvas(CanvasData::DrawingArea(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct DrawingAreaData {}

impl DrawingAreaData {
    pub fn new() -> Self {
        DrawingAreaData {}
    }
}

#[derive(Debug, PartialEq, Clone)]
enum CanvasKind {
    GLArea,
    DrawingArea,
}

#[derive(Debug, PartialEq, Clone)]
enum CanvasData {
    GLArea(GLAreaData),
    DrawingArea(DrawingAreaData),
}

impl DataTrait for CanvasData {
    fn build(&self, _: &gtk4::Box) -> Widget {
        match &self {
            CanvasData::GLArea(_) => gtk4::GLArea::builder().build().into(),
            CanvasData::DrawingArea(_) => gtk4::DrawingArea::builder().build().into(),
        }
    }
}
/* #endregion Canvases */
/* #region Loaders */
fn spinner(attributes: Attributes) -> Option<DataEnum> {
    let mut data = SpinnerData::new();
    for attr in attributes {
        match attr.name() {
            "spin" | "spinning" => match attr.value() {
                "f" | "false" | "n" | "no" => data.spinning = false,
                _ => data.spinning = true,
            },
            _ => {}
        }
    }
    Some(DataEnum::Loader(LoaderData::Spinner(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct SpinnerData {
    spinning: bool,
}

impl SpinnerData {
    pub fn new() -> Self {
        SpinnerData { spinning: true }
    }
}

fn level_bar(attributes: Attributes) -> Option<DataEnum> {
    let mut data = LevelBarData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "progress" | "value" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.progress = val;
                }
            }
            "min" | "minimum" | "floor" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.min = val;
                }
            }
            "max" | "maximum" | "ceiling" | "ceil" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.max = val;
                }
            }
            "mode" | "type" => match val {
                "discrete" | "step" | "stepped" => data.mode = gtk4::LevelBarMode::Discrete,
                _ => data.mode = gtk4::LevelBarMode::Continuous,
            },
            "discrete" => match val {
                "false" | "f" | "no" | "n" => data.mode = gtk4::LevelBarMode::Continuous,
                _ => data.mode = gtk4::LevelBarMode::Discrete,
            },
            "invert" | "inverted" | "rev" | "reversed" => match val {
                "false" | "f" | "no" | "n" => data.inverted = false,
                _ => data.inverted = true,
            },
            _ => {}
        }
    }
    Some(DataEnum::Loader(LoaderData::LevelBar(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct LevelBarData {
    progress: f64,
    min: f64,
    max: f64,
    mode: gtk4::LevelBarMode,
    inverted: bool,
}

impl LevelBarData {
    pub fn new() -> Self {
        LevelBarData {
            progress: 0f64,
            min: 0f64,
            max: 100f64,
            mode: gtk4::LevelBarMode::Continuous,
            inverted: false,
        }
    }
}

fn progress_bar(children: Children, attributes: Attributes, parent: &BoxData) -> Option<DataEnum> {
    let mut data = ProgressBarData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "progress" | "value" => {
                if let Ok(val) = val.parse::<f64>() {
                    if (0f64..=1f64).contains(&val) {
                        data.progress = val;
                    }
                }
            }
            "ellipse" | "ellipsize" => match val {
                "start" | "left" | "front" | "beginning" => {
                    data.ellipsize = gtk4::pango::EllipsizeMode::Start;
                }
                "end" | "right" | "back" => {
                    data.ellipsize = gtk4::pango::EllipsizeMode::End;
                }
                "middle" | "center" | "centre" => {
                    data.ellipsize = gtk4::pango::EllipsizeMode::Middle;
                }
                _ => data.ellipsize = gtk4::pango::EllipsizeMode::None,
            },
            "pulse" | "bounce" => {
                if let Ok(val) = val.parse::<f64>() {
                    if (0f64..=1f64).contains(&val) {
                        data.pulse = val;
                    }
                }
            }
            "invert" | "inverted" | "rev" | "reversed" => match val {
                "false" | "f" | "no" | "n" => data.inverted = false,
                _ => data.inverted = true,
            },
            _ => {}
        }
    }
    for child in children {
        if let Some(WidgetData {
            data: DataEnum::Label(label),
            ..
        }) = process_element(&child, parent)
        {
            data.text.push_str(&label.concat());
        }
    }
    Some(DataEnum::Loader(LoaderData::ProgressBar(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct ProgressBarData {
    ellipsize: gtk4::pango::EllipsizeMode,
    progress: f64,
    inverted: bool,
    pulse: f64,
    text: String,
}

impl ProgressBarData {
    pub fn new() -> Self {
        ProgressBarData {
            ellipsize: gtk4::pango::EllipsizeMode::None,
            progress: 0f64,
            inverted: false,
            pulse: 0f64,
            text: String::new(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum LoaderKind {
    Spinner,
    LevelBar,
    ProgressBar,
}
#[derive(Debug, PartialEq, Clone)]
enum LoaderData {
    Spinner(SpinnerData),
    LevelBar(LevelBarData),
    ProgressBar(ProgressBarData),
}

impl DataTrait for LoaderData {
    fn build(&self, _: &gtk4::Box) -> Widget {
        match &self {
            LoaderData::Spinner(loader) => gtk4::Spinner::builder()
                .spinning(loader.spinning)
                .build()
                .into(),
            LoaderData::LevelBar(loader) => gtk4::LevelBar::builder()
                .value(loader.progress)
                .min_value(loader.min)
                .max_value(loader.max)
                .mode(loader.mode)
                .inverted(loader.inverted)
                .build()
                .into(),
            LoaderData::ProgressBar(loader) => gtk4::ProgressBar::builder()
                .fraction(loader.progress)
                .inverted(loader.inverted)
                .pulse_step(loader.pulse)
                .text(&loader.text)
                .ellipsize(loader.ellipsize)
                .show_text(!loader.text.is_empty())
                .build()
                .into(),
        }
    }
}
/* #endregion Loaders */
/* #region Inputs */
fn process_textview(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<DataEnum> {
    let mut data = TextViewData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "readonly" | "ro" | "locked" => match val {
                "false" | "f" | "no" | "n" => data.editable = true,
                _ => data.editable = false,
            },
            "wrap" | "wrapmode" => match val {
                "c" | "char" => data.wrap_mode = gtk4::WrapMode::Char,
                "w" | "word" => data.wrap_mode = gtk4::WrapMode::Word,
                "wc" | "wordchar" | "both" => data.wrap_mode = gtk4::WrapMode::WordChar,
                _ => data.wrap_mode = gtk4::WrapMode::None,
            },
            "justification" | "align" => match val {
                "l" | "start" | "left" | "front" | "beginning" => {
                    data.align = gtk4::Justification::Left
                }
                "r" | "end" | "right" | "back" => data.align = gtk4::Justification::Right,
                "e" | "center" | "middle" | "centre" => data.align = gtk4::Justification::Center,
                "j" | "justify" | "fill" => data.align = gtk4::Justification::Fill,
                _ => {}
            },
            "indent" => {
                if let Ok(val) = val.parse::<i32>() {
                    data.indent = val;
                }
            }
            "cursor" | "caret" => match val {
                "true" | "t" | "yes" | "y" => data.cursor = true,
                _ => data.cursor = false,
            },
            "monospace" | "mono" => match val {
                "false" | "f" | "no" | "n" => data.monospace = false,
                _ => data.monospace = true,
            },
            _ => {}
        }
    }
    for child in children {
        if let Some(WidgetData {
            data: DataEnum::Label(label),
            ..
        }) = process_element(&child, parent)
        {
            data.buffer.push_str(&label.concat());
        }
    }
    Some(DataEnum::Input(InputData::TextView(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct TextViewData {
    buffer: String,
    editable: bool,
    wrap_mode: gtk4::WrapMode,
    align: gtk4::Justification,
    indent: i32,
    cursor: bool,
    monospace: bool,
}

impl TextViewData {
    pub fn new() -> Self {
        TextViewData {
            buffer: String::new(),
            editable: true,
            wrap_mode: gtk4::WrapMode::None,
            align: gtk4::Justification::Left,
            indent: 0,
            cursor: true,
            monospace: false,
        }
    }
}

fn process_entry(attributes: Attributes) -> Option<DataEnum> {
    let mut data = EntryData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "text" => data.text = val.to_string(),
            "hint" | "placeholder" => data.hint = val.to_string(),
            "max" | "length" | "len" => {
                if let Ok(val) = val.parse::<i32>() {
                    data.max = val;
                }
            }
            _ => {}
        }
    }
    Some(DataEnum::Input(InputData::Entry(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct EntryData {
    text: String,
    hint: String,
    max: i32,
}

impl EntryData {
    pub fn new() -> Self {
        EntryData {
            text: String::new(),
            hint: String::new(),
            max: 0,
        }
    }
}

fn search_entry(attributes: Attributes) -> Option<DataEnum> {
    let mut data = SearchEntryData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "text" => data.text = val.to_string(),
            "hint" | "placeholder" => data.hint = val.to_string(),
            "activatable" | "trigger" => match val {
                "true" | "t" | "yes" | "y" => data.activate = true,
                _ => data.activate = false,
            },
            _ => {}
        }
    }
    Some(DataEnum::Input(InputData::SearchEntry(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct SearchEntryData {
    text: String,
    hint: String,
    activate: bool,
}

impl SearchEntryData {
    pub fn new() -> Self {
        SearchEntryData {
            text: String::new(),
            hint: String::new(),
            activate: true,
        }
    }
}

fn password_entry(attributes: Attributes) -> Option<DataEnum> {
    let mut data = PasswordEntryData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "text" => data.text = val.to_string(),
            "hint" | "placeholder" => data.hint = val.to_string(),
            "icon" | "peek" => match val {
                "true" | "t" | "yes" | "y" => data.icon = true,
                _ => data.icon = false,
            },
            "activatable" | "trigger" => match val {
                "true" | "t" | "yes" | "y" => data.activate = true,
                _ => data.activate = false,
            },
            _ => {}
        }
    }
    Some(DataEnum::Input(InputData::PasswordEntry(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct PasswordEntryData {
    text: String,
    hint: String,
    icon: bool,
    activate: bool,
}

impl PasswordEntryData {
    pub fn new() -> Self {
        PasswordEntryData {
            text: String::new(),
            hint: String::new(),
            icon: true,
            activate: true,
        }
    }
}

fn spin_button(attributes: Attributes) -> Option<DataEnum> {
    let mut data = SpinButtonData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "value" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.value = val;
                }
            }
            "range" => {
                if let Some((Ok(min), Ok(max))) = val
                    .split_once(',')
                    .map(|(x, y)| (x.parse::<f64>(), y.parse::<f64>()))
                {
                    data.range = (min, max);
                }
            }
            "min" | "minimum" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.range.0 = val;
                }
            }
            "max" | "maximum" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.range.1 = val;
                }
            }
            "inc" | "increment" => {
                if let Some((Ok(min), Ok(max))) = val
                    .split_once(',')
                    .map(|(x, y)| (x.parse::<f64>(), y.parse::<f64>()))
                {
                    data.increments = (min, max);
                }
            }
            "step" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.increments.0 = val;
                }
            }
            "page" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.increments.1 = val;
                }
            }
            "numeric" | "number" => match val {
                "true" | "t" | "yes" | "y" => data.numeric = true,
                _ => data.numeric = false,
            },
            "wrap" | "loop" => match val {
                "false" | "f" | "no" | "n" => data.wrap = false,
                _ => data.wrap = true,
            },
            "rate" | "acceleration" => {
                if let Ok(val) = val.parse::<f64>() {
                    data.rate = val;
                }
            }
            "snap" | "lock" => match val {
                "false" | "f" | "no" | "n" => data.snap = false,
                _ => data.snap = true,
            },
            _ => {}
        }
    }
    Some(DataEnum::Input(InputData::Spin(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct SpinButtonData {
    value: f64,
    range: (f64, f64),
    increments: (f64, f64),
    numeric: bool,
    wrap: bool,
    rate: f64,
    snap: bool,
}

impl SpinButtonData {
    pub fn new() -> Self {
        SpinButtonData {
            value: 0f64,
            range: (0f64, 100f64),
            increments: (1f64, 5f64),
            numeric: true,
            wrap: false,
            rate: 0f64,
            snap: false,
        }
    }
}

fn editable_label(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<DataEnum> {
    let mut data = EditableLabelData::new();
    for attr in attributes {
        let val = attr.value();
        match attr.name() {
            "editable" | "edit" => match val {
                "true" | "t" | "yes" | "y" => data.editable = true,
                _ => data.editable = false,
            },
            _ => {}
        }
    }
    for child in children {
        if let Some(WidgetData {
            data: DataEnum::Label(label),
            ..
        }) = process_element(&child, parent)
        {
            data.text.push_str(&label.concat());
        }
    }
    Some(DataEnum::Input(InputData::Editable(data)))
}

#[derive(Debug, PartialEq, Clone)]
struct EditableLabelData {
    text: String,
    editable: bool,
}

impl EditableLabelData {
    pub fn new() -> Self {
        EditableLabelData {
            text: String::new(),
            editable: true,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum InputKind {
    TextView,
    Entry,
    Search,
    Password,
    Spin,
    Editable,
}

#[derive(Debug, PartialEq, Clone)]
enum InputData {
    TextView(TextViewData),
    Entry(EntryData),
    SearchEntry(SearchEntryData),
    PasswordEntry(PasswordEntryData),
    Spin(SpinButtonData),
    Editable(EditableLabelData),
}

impl DataTrait for InputData {
    fn build(&self, _: &gtk4::Box) -> Widget {
        match &self {
            InputData::TextView(input) => {
                let buffer = gtk4::TextBuffer::new(None);
                buffer.set_text(&input.buffer);
                gtk4::TextView::builder()
                    .buffer(&buffer)
                    .editable(input.editable)
                    .wrap_mode(input.wrap_mode)
                    .justification(input.align)
                    .indent(input.indent)
                    .cursor_visible(input.cursor)
                    .monospace(input.monospace)
                    .build()
                    .into()
            }
            InputData::Entry(input) => gtk4::Entry::builder()
                .text(&input.text)
                .placeholder_text(&input.hint)
                .max_length(input.max)
                .build()
                .into(),
            InputData::SearchEntry(input) => gtk4::SearchEntry::builder()
                .text(&input.text)
                .placeholder_text(&input.hint)
                .activates_default(input.activate)
                .build()
                .into(),
            InputData::PasswordEntry(input) => gtk4::PasswordEntry::builder()
                .text(&input.text)
                .placeholder_text(&input.hint)
                .show_peek_icon(input.icon)
                .activates_default(input.activate)
                .build()
                .into(),
            InputData::Spin(input) => {
                let spin = gtk4::SpinButton::builder()
                    .numeric(input.numeric)
                    .wrap(input.wrap)
                    .climb_rate(input.rate)
                    .snap_to_ticks(input.snap)
                    .build();
                spin.set_range(input.range.0, input.range.1);
                spin.set_increments(input.increments.0, input.increments.1);
                spin.set_value(input.value);
                spin.into()
            }
            InputData::Editable(input) => gtk4::EditableLabel::builder()
                .text(&input.text)
                .editable(input.editable)
                .build()
                .into(),
        }
    }
}
/* #endregion Inputs */
/* #region Clones */
fn process_cloned(attributes: Attributes, parent: &BoxData) -> Option<WidgetData> {
    let mut old_name = None;
    let mut new_name = None;
    for attr in attributes {
        let val = attr.value();
        if !val.trim().is_empty() {
            match attr.name() {
                "object" | "from" | "import" | "src" | "source" => {
                    old_name = Some(val);
                }
                "subject" | "to" | "as" | "dest" | "destination" => {
                    new_name = Some(val);
                }
                _ => {}
            }
        }
    }
    if old_name == new_name {
        return None;
    }
    if let Some(old_name) = old_name {
        //     for cur_child in &parent.children {
        //         if cur_child.get_name() == old_name {
        //             let mut new_child = cur_child.clone();
        //             new_child.rem_shadow();
        //             if let Some(new_name) = new_name {
        //                 new_child.set_name(new_name);
        //             }
        //             return Some(WidgetData {
        //                 defaults: WidgetDefaults::new(),
        //                 data: DataEnum::Clone(Box::new(new_child)),
        //             });
        //         }
        //     }
        if let Some(mut child) = get_clone(old_name, parent) {
            if let Some(new_name) = new_name {
                child.set_name(new_name);
            }
            Some(WidgetData {
                defaults: WidgetDefaults::new(),
                data: DataEnum::Clone(Box::new(child)),
            })
        } else {
            None
        }
    } else {
        None
    }
}

fn get_clone(old_name: &str, parent: &BoxData) -> Option<WidgetData> {
    for child in &parent.children {
        if child.get_name() == old_name {
            let mut child = child.clone();
            child.rem_shadow();
            return Some(child);
        }
    }
    None
}

/* #endregion Clones */
/* #region Globals */
#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
struct WidgetDefaults {
    shadow: bool,
    halign: gtk4::Align,
    valign: gtk4::Align,
    hexpand: bool,
    vexpand: bool,
    tooltip: Option<gtk4::glib::GString>,
    opacity: f64,
    margin: Margin,
    name: String,
    size_req: Option<(i32, i32)>,
    orientation: gtk4::Orientation,
    overflow: gtk4::Overflow,
    focusable: bool,
}

impl WidgetDefaults {
    pub fn new() -> Self {
        Self {
            shadow: false,
            halign: gtk4::Align::Start,
            valign: gtk4::Align::Start,
            hexpand: false,
            vexpand: false,
            tooltip: None,
            opacity: 1f64,
            margin: Margin::new(),
            name: String::new(),
            size_req: None,
            orientation: gtk4::Orientation::Horizontal,
            overflow: gtk4::Overflow::Visible,
            focusable: true,
        }
    }
    pub fn modify(&mut self, attr: roxmltree::Attribute, parent: &BoxData) {
        let val = attr.value();
        match attr.name() {
            "_shadow" | "_ref" => match val {
                "false" | "f" | "no" | "n" => self.shadow = false,
                _ => self.shadow = true,
            },
            "_halign" => match val {
                "fill" => self.halign = gtk4::Align::Fill,
                "start" | "left" | "front" | "beginning" => self.halign = gtk4::Align::Start,
                "end" | "right" | "back" => self.halign = gtk4::Align::End,
                "center" | "middle" | "centre" => self.halign = gtk4::Align::Center,
                "baseline" | "base" => self.halign = gtk4::Align::Baseline,
                _ => {}
            },
            "_valign" => match val {
                "fill" => self.valign = gtk4::Align::Fill,
                "start" | "left" | "front" | "beginning" => self.valign = gtk4::Align::Start,
                "end" | "right" | "back" => self.valign = gtk4::Align::End,
                "center" | "middle" | "centre" => self.valign = gtk4::Align::Center,
                "baseline" | "base" => self.valign = gtk4::Align::Baseline,
                _ => {}
            },
            "_hexpand" => match val {
                "false" | "f" | "no" | "n" => self.hexpand = false,
                _ => self.hexpand = true,
            },
            "_vexpand" => match val {
                "false" | "f" | "no" | "n" => self.vexpand = false,
                _ => self.vexpand = true,
            },
            "_tooltip" => {
                if !val.trim().is_empty() {
                    if let Some(val) = val.strip_prefix(":") {
                        if let Some(WidgetData {
                            data: DataEnum::Label(label),
                            ..
                        }) = get_clone(val, parent)
                        {
                            self.tooltip = Some(label.compile());
                        }
                    } else if let Some(val) = val.strip_prefix("\\:") {
                        self.tooltip = Some(format!(":{}", attr_escape(val)).into());
                    } else {
                        self.tooltip = Some(attr_escape(val).into())
                    }
                }
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
            "_name" | "_" => self.name = val.to_string(),
            "_size" | "_size_req" => {
                if let Some((Ok(width), Ok(height))) = val
                    .split_once(',')
                    .map(|x| (x.0.parse::<i32>(), x.1.parse::<i32>()))
                {
                    self.size_req = Some((width, height));
                }
            }
            "_orientation" => match val {
                "v" | "vert" | "vertical" => self.orientation = gtk4::Orientation::Vertical,
                _ => self.orientation = gtk4::Orientation::Horizontal,
            },
            "_overflow" | "_leak" | "_of" => match val {
                "false" | "f" | "no" | "n" => self.overflow = gtk4::Overflow::Hidden,
                _ => self.overflow = gtk4::Overflow::Visible,
            },
            "_clickthrough" | "_ghost" => match val {
                "false" | "f" | "no" | "n" => self.focusable = true,
                _ => self.focusable = false,
            },
            _ => {}
        }
    }
    pub fn apply(&self, widget: &impl IsA<Widget>) {
        widget.set_hexpand(self.hexpand);
        widget.set_vexpand(self.vexpand);
        widget.set_halign(self.halign);
        widget.set_valign(self.valign);
        widget.set_tooltip_markup(self.tooltip.as_deref());
        widget.set_opacity(self.opacity);
        widget.set_margin_top(self.margin.top);
        widget.set_margin_bottom(self.margin.bottom);
        widget.set_margin_start(self.margin.start);
        widget.set_margin_end(self.margin.end);
        widget.set_widget_name(&self.name);
        widget.set_can_target(self.focusable);
        widget.set_can_focus(self.focusable);
        if let Some((width, height)) = &self.size_req {
            widget.set_size_request(*width, *height);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ElemKind {
    Label(Text),
    Container(ContainerKind),
    Button(ButtonKind),
    Canvas(CanvasKind),
    Loader(LoaderKind),
    Input(InputKind),
    Cloned,
    Fallback,
}

#[derive(Debug, PartialEq, Clone)]
enum DataEnum {
    Label(Box<LabelData>),
    Container(ContainerData),
    Button(ButtonData),
    Canvas(CanvasData),
    Loader(LoaderData),
    Input(InputData),
    Clone(Box<WidgetData>),
}

pub trait DataTrait {
    fn build(&self, parent: &gtk4::Box) -> Widget;
}

impl DataTrait for DataEnum {
    fn build(&self, parent: &gtk4::Box) -> Widget {
        match &self {
            DataEnum::Label(label) => label.build(parent),
            DataEnum::Container(container) => container.build(parent),
            DataEnum::Button(button) => button.build(parent),
            DataEnum::Canvas(canvas) => canvas.build(parent),
            DataEnum::Loader(loader) => loader.build(parent),
            DataEnum::Input(input) => input.build(parent),
            DataEnum::Clone(clone) => clone.build(parent),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct WidgetData {
    defaults: WidgetDefaults,
    data: DataEnum,
}

impl WidgetData {
    pub fn build(&self, parent: &gtk4::Box) -> Widget {
        let widget = self.data.build(parent);
        self.defaults.apply(&widget);
        widget
    }
    pub fn get_name(&self) -> String {
        self.defaults.name.to_string()
    }
    pub fn set_name(&mut self, new_name: &str) {
        self.defaults.name = new_name.to_string();
    }
    pub fn get_shadow(&self) -> bool {
        self.defaults.shadow
    }
    pub fn rem_shadow(&mut self) {
        self.defaults.shadow = false;
    }
}

impl DataTrait for WidgetData {
    fn build(&self, parent: &gtk4::Box) -> Widget {
        let widget = self.build(parent);
        self.defaults.apply(&widget);
        widget
    }
}
/* #endregion Globals */
