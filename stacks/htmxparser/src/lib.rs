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
    let mut data = BoxData::new();
    data.defaults.hexpand = true;
    data.defaults.vexpand = true;
    data.defaults.halign = gtk4::Align::Fill;
    data.defaults.valign = gtk4::Align::Fill;
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
    data.build()
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
        "grid" => ElemKind::Container(BoxKind::Grid),
        "griditem" | "gi" => ElemKind::Container(BoxKind::GridItem),
        "div" | "box" => ElemKind::Container(BoxKind::Normal),
        "button" | "btn" => ElemKind::Button(ButtonKind::Normal),
        "toggle" | "tbtn" => ElemKind::Button(ButtonKind::Toggle),
        "checked" | "check" | "cbtn" | "radio" | "rbtn" => ElemKind::Button(ButtonKind::Checked),
        "canvas" | "draw" | "drawingarea" => ElemKind::Canvas(CanvasKind::DrawingArea),
        "gl" | "glarea" => ElemKind::Canvas(CanvasKind::GLArea),
        "clone" | "cloned" => ElemKind::Cloned,
        "spinner" | "spin" => ElemKind::Loader(LoaderKind::Spinner),
        "levelbar" | "lb" => ElemKind::Loader(LoaderKind::LevelBar),
        "progressbar" | "pb" => ElemKind::Loader(LoaderKind::ProgressBar),
        _ => ElemKind::Fallback,
    }
}

fn process_element(elem: &Node, parent: &BoxData) -> Option<WidgetData> {
    let kind = derive_kind(elem.tag_name().name());
    match kind {
        ElemKind::Label(kind) => process_label(&kind, elem.children(), elem.attributes()),
        ElemKind::Container(BoxKind::Grid) => {
            process_grid(elem.children(), elem.attributes(), parent)
        }
        ElemKind::Container(BoxKind::GridItem) => None,
        ElemKind::Container(BoxKind::Normal) => process_box(elem.children(), elem.attributes()),
        ElemKind::Button(kind) => match kind {
            ButtonKind::Normal => normal_button(elem.children(), elem.attributes(), parent),
            ButtonKind::Toggle => toggle_button(elem.children(), elem.attributes(), parent),
            ButtonKind::Checked => check_button(elem.children(), elem.attributes(), parent),
        },
        ElemKind::Canvas(CanvasKind::DrawingArea) => drawing_area(elem.attributes()),
        ElemKind::Canvas(CanvasKind::GLArea) => gl_area(elem.attributes()),
        ElemKind::Cloned => {
            if !elem.has_children() {
                process_cloned(elem.attributes(), parent)
            } else {
                None
            }
        }
        ElemKind::Fallback => elem
            .text()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|text| {
                let mut data = LabelData::new();
                data.text = text.to_string();
                WidgetData::Label(Box::new(data))
            }),
        ElemKind::Loader(loader) => {
            process_loader(&loader, elem.attributes(), elem.children(), parent)
        }
    }
}

/* #region Labels */
fn process_label(kind: &Text, children: Children, attributes: Attributes) -> Option<WidgetData> {
    Some(WidgetData::Label(Box::new(process_text(
        kind, children, attributes,
    )?)))
}

fn process_text(kind: &Text, children: Children, attributes: Attributes) -> Option<LabelData> {
    let mut data = LabelData::new();
    match kind {
        Text::Kind(kind) => {
            data = text_kind(kind, children, attributes, None)?;
        }
        Text::Style(style) => data.text = raw_text_style(style, children)?,
    }
    Some(data)
}

fn text_kind(
    kind: &TextKind,
    children: Children,
    attributes: Attributes,
    dad: Option<&LabelData>,
) -> Option<LabelData> {
    let mut data = text_attributes(attributes, dad);
    for child in children {
        match child.node_type() {
            NodeType::Element => {
                if let ElemKind::Label(text) = derive_kind(child.tag_name().name()) {
                    match text {
                        Text::Kind(kind) => {
                            if let Some(cur_data) =
                                text_kind(&kind, child.children(), child.attributes(), Some(&data))
                            {
                                if data.text.is_empty() {
                                    data = cur_data;
                                } else {
                                    data.children.push(cur_data);
                                }
                            }
                        }
                        Text::Style(style) => {
                            if let Some(text) = raw_text_style(&style, child.children()) {
                                if data.text.is_empty() {
                                    data.text = text;
                                } else {
                                    let mut raw_data = data.clone();
                                    raw_data.children = Vec::new();
                                    raw_data.text = text;
                                    data.children.push(raw_data);
                                }
                            }
                        }
                    }
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
            "stretch" => match val {
                "uc" | "ultracondensed" => data.stretch = Some(l_attr::Stretch::UltraCondensed),
                "ec" | "extracondensed" => data.stretch = Some(l_attr::Stretch::ExtraCondensed),
                "c" | "condensed" => data.stretch = Some(l_attr::Stretch::Condensed),
                "sc" | "semicondensed" => data.stretch = Some(l_attr::Stretch::SemiCondensed),
                "se" | "semiexpanded" => data.stretch = Some(l_attr::Stretch::SemiExpanded),
                "e" | "expanded" => data.stretch = Some(l_attr::Stretch::Expanded),
                "ee" | "extraexpanded" => data.stretch = Some(l_attr::Stretch::ExtraExpanded),
                "ue" | "ultraexpanded" => data.stretch = Some(l_attr::Stretch::UltraExpanded),
                _ => data.stretch = Some(l_attr::Stretch::Normal),
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
            "baseline_shift" | "fall" => {
                if val.strip_suffix("pt").unwrap_or(val).parse::<i32>().is_ok() {
                    data.fall = Some(val.to_string());
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
            "letter_spacing" | "spacing" => {
                if let Ok(val) = val.strip_suffix("pt").unwrap_or(val).parse::<f64>() {
                    if val >= 0f64 {
                        data.spacing = Some(val);
                    }
                }
            }
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
            _ => data.defaults.modify(attr),
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
    pub enum Stretch {
        UltraCondensed,
        ExtraCondensed,
        Condensed,
        SemiCondensed,
        Normal,
        SemiExpanded,
        Expanded,
        ExtraExpanded,
        UltraExpanded,
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
    defaults: WidgetDefaults,
    link: Option<String>,
    font: Option<String>,
    face: Option<String>,
    size: Option<String>,
    style: Option<l_attr::Style>,
    weight: Option<l_attr::Weight>,
    variant: Option<l_attr::Variant>,
    stretch: Option<l_attr::Stretch>,
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
    fall: Option<String>,
    scale: Option<l_attr::Scale>,
    strikethrough: Option<bool>,
    scolor: Option<String>,
    fallback: Option<bool>,
    lang: Option<String>,
    spacing: Option<f64>,
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
            defaults: WidgetDefaults::new(),
            link: None,
            font: None,
            face: None,
            size: None,
            style: None,
            weight: None,
            variant: None,
            stretch: None,
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
            fall: None,
            scale: None,
            strikethrough: None,
            scolor: None,
            fallback: None,
            lang: None,
            spacing: None,
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
    pub fn compile(&self) -> String {
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
        if let Some(stretch) = &self.stretch {
            a.push_str(&format!(
                "stretch='{}' ",
                match *stretch {
                    l_attr::Stretch::UltraCondensed => "ultracondensed",
                    l_attr::Stretch::ExtraCondensed => "extracondensed",
                    l_attr::Stretch::Condensed => "condensed",
                    l_attr::Stretch::SemiCondensed => "semicondensed",
                    l_attr::Stretch::Normal => "normal",
                    l_attr::Stretch::SemiExpanded => "semiexpanded",
                    l_attr::Stretch::Expanded => "expanded",
                    l_attr::Stretch::ExtraExpanded => "extraexpanded",
                    l_attr::Stretch::UltraExpanded => "ultraexpanded",
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
        if let Some(fall) = &self.fall {
            a.push_str(&format!("baseline_shift='{fall}' "));
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
        if let Some(spacing) = &self.spacing {
            a.push_str(&format!("letter_spacing='{spacing}pt' "));
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
    }
    pub fn build(&self) -> Widget {
        let markup = self.compile();
        let label = gtk4::Label::builder()
            .use_markup(true)
            .label(markup)
            .build();
        self.defaults.apply(&label);
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
fn _attr_escape(raw: &str) -> String {
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
    // Link,
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
fn process_box(children: Children, attributes: Attributes) -> Option<WidgetData> {
    let mut data = BoxData::new();
    for attr in attributes {
        match attr.name() {
            "orientation" | "align" => match attr.value() {
                "horizontal" | "h" => data.orientation = gtk4::Orientation::Horizontal,
                _ => data.orientation = gtk4::Orientation::Vertical,
            },
            _ => data.defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(child) = process_element(&child, &data) {
            data.children.push(child);
        }
    }
    Some(WidgetData::Box(data))
}

#[derive(Debug, PartialEq, Clone)]
struct BoxData {
    defaults: WidgetDefaults,
    orientation: gtk4::Orientation,
    children: Vec<WidgetData>,
}

impl BoxData {
    pub fn new() -> Self {
        BoxData {
            defaults: WidgetDefaults::new(),
            orientation: gtk4::Orientation::Vertical,
            children: Vec::new(),
        }
    }
    pub fn build(&self) -> gtk4::Box {
        let widget = gtk4::Box::builder().orientation(self.orientation).build();
        self.defaults.apply(&widget);
        for child in &self.children {
            widget.append(&child.build(&widget));
        }
        widget
    }
}

fn process_grid(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<WidgetData> {
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
            _ => data.defaults.modify(attr),
        }
    }
    for child in children {
        if derive_kind(child.tag_name().name()) != ElemKind::Container(BoxKind::GridItem) {
            println!("Expected GridItem, found: {child:?}");
            continue;
        }
        let mut loc = (0i32, 0i32, 0i32, 0i32);
        for attr in child.attributes() {
            let val = attr.value();
            match attr.name() {
                "c" | "column" => {
                    if let Ok(val) = val.parse::<i32>() {
                        loc.0 = val;
                    }
                }
                "r" | "row" => {
                    if let Ok(val) = val.parse::<i32>() {
                        loc.1 = val;
                    }
                }
                "w" | "width" => {
                    if let Ok(val) = val.parse::<i32>() {
                        loc.2 = val;
                    }
                }
                "h" | "height" => {
                    if let Ok(val) = val.parse::<i32>() {
                        loc.3 = val;
                    }
                }
                _ => {}
            };
        }
        for child in child.children() {
            if child.is_text() && child.text().is_some_and(|x| x.trim().is_empty()) {
                continue;
            } else if let Some(widget) = process_element(&child, parent) {
                data.children.push((widget, loc));
                break;
            }
        }
    }
    Some(WidgetData::Grid(data))
}

#[derive(Debug, PartialEq, Clone)]
enum BoxKind {
    Grid,
    GridItem,
    Normal,
}

#[derive(Debug, PartialEq, Clone)]
struct GridData {
    defaults: WidgetDefaults,
    col_hom: bool,
    row_hom: bool,
    children: Vec<(WidgetData, (i32, i32, i32, i32))>,
}

impl GridData {
    pub fn new() -> Self {
        GridData {
            defaults: WidgetDefaults::new(),
            col_hom: true,
            row_hom: true,
            children: Vec::new(),
        }
    }
    pub fn build(&self, parent: &gtk4::Box) -> gtk4::Grid {
        let widget = gtk4::Grid::builder()
            .column_homogeneous(self.col_hom)
            .row_homogeneous(self.row_hom)
            .build();
        self.defaults.apply(&widget);
        for child in &self.children {
            let loc = child.1;
            widget.attach(&child.0.build(parent), loc.0, loc.1, loc.2, loc.3);
        }
        widget
    }
}

/* #endregion Containers */
/* #region Buttons */
fn normal_button(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<WidgetData> {
    let mut data = ButtonData::new();
    for attr in attributes {
        data.defaults.modify(attr);
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(WidgetData::Button(data))
}

#[derive(Debug, PartialEq, Clone)]
struct ButtonData {
    defaults: WidgetDefaults,
    child: Option<Box<WidgetData>>,
}

impl ButtonData {
    pub fn new() -> Self {
        ButtonData {
            defaults: WidgetDefaults::new(),
            child: None,
        }
    }
    pub fn build(&self, parent: &gtk4::Box) -> gtk4::Button {
        let widget = gtk4::Button::builder().build();
        self.defaults.apply(&widget);
        if let Some(child) = &self.child {
            let child = child.build(parent);
            widget.set_child(Some(&child));
        }
        widget
    }
}

fn toggle_button(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<WidgetData> {
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
            _ => data.defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(WidgetData::ToggleButton(data))
}

#[derive(Debug, PartialEq, Clone)]
struct ToggleButtonData {
    defaults: WidgetDefaults,
    checked: bool,
    group: Option<String>,
    child: Option<Box<WidgetData>>,
}

impl ToggleButtonData {
    pub fn new() -> Self {
        ToggleButtonData {
            defaults: WidgetDefaults::new(),
            checked: false,
            group: None,
            child: None,
        }
    }
    pub fn build(&self, parent: &gtk4::Box) -> gtk4::ToggleButton {
        let widget = gtk4::ToggleButton::builder().active(self.checked).build();
        self.defaults.apply(&widget);
        if let Some(group) = &self.group {
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
        if let Some(child) = &self.child {
            let child = child.build(parent);
            widget.set_child(Some(&child));
        }
        widget
    }
}

fn check_button(
    children: Children,
    attributes: Attributes,
    parent: &BoxData,
) -> Option<WidgetData> {
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
            _ => data.defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(widget) = process_element(&child, parent) {
            data.child = Some(Box::from(widget));
            break;
        }
    }
    Some(WidgetData::CheckButton(data))
}

#[derive(Debug, PartialEq, Clone)]
struct CheckButtonData {
    defaults: WidgetDefaults,
    checked: bool,
    group: Option<String>,
    child: Option<Box<WidgetData>>,
}

impl CheckButtonData {
    pub fn new() -> Self {
        CheckButtonData {
            defaults: WidgetDefaults::new(),
            checked: false,
            group: None,
            child: None,
        }
    }
    pub fn build(&self, parent: &gtk4::Box) -> gtk4::CheckButton {
        let widget = gtk4::CheckButton::builder().build();
        widget.set_active(self.checked);
        self.defaults.apply(&widget);
        if let Some(group) = &self.group {
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
        if let Some(child) = &self.child {
            let child = child.build(parent);
            // POV when gtk4-rs forgot to implement ButtonExt for CheckButton, so you
            // have to implement `CheckButton::set_child(Option<&impl IsA<Widget>)` yourself:
            widget.set_property("child", Some(&child));
        }
        widget
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ButtonKind {
    Normal,
    Toggle,
    Checked,
}
/* #endregion Buttons */
/* #region Canvases */
fn gl_area(attributes: Attributes) -> Option<WidgetData> {
    let mut data = GLAreaData::new();
    for attr in attributes {
        data.defaults.modify(attr);
    }
    Some(WidgetData::GLArea(data))
}

#[derive(Debug, PartialEq, Clone)]
struct GLAreaData {
    defaults: WidgetDefaults,
}

impl GLAreaData {
    pub fn new() -> Self {
        GLAreaData {
            defaults: WidgetDefaults::new(),
        }
    }
    pub fn build(&self) -> gtk4::GLArea {
        let widget = gtk4::GLArea::builder().build();
        self.defaults.apply(&widget);
        widget
    }
}

fn drawing_area(attributes: Attributes) -> Option<WidgetData> {
    let mut data = DrawingAreaData::new();
    for attr in attributes {
        data.defaults.modify(attr);
    }
    Some(WidgetData::DrawingArea(data))
}

#[derive(Debug, PartialEq, Clone)]
struct DrawingAreaData {
    defaults: WidgetDefaults,
}

impl DrawingAreaData {
    pub fn new() -> Self {
        DrawingAreaData {
            defaults: WidgetDefaults::new(),
        }
    }
    pub fn build(&self) -> gtk4::DrawingArea {
        let widget = gtk4::DrawingArea::builder().build();
        self.defaults.apply(&widget);
        widget
    }
}

#[derive(Debug, PartialEq, Clone)]
enum CanvasKind {
    GLArea,
    DrawingArea,
}
/* #endregion Canvases */
/* #region Clones */
fn process_cloned(attributes: Attributes, parent: &BoxData) -> Option<WidgetData> {
    let mut old_name = None;
    let mut new_name = None;
    for attr in attributes {
        let val = attr.value().trim();
        if !val.is_empty() {
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
        for cur_child in &parent.children {
            if cur_child.get_name() == old_name {
                let mut new_child = cur_child.clone();
                if let Some(new_name) = new_name {
                    new_child.set_name(new_name);
                }
                return Some(WidgetData::Clone(Box::new(new_child)));
            }
        }
    }
    None
}

/* #endregion Clones */
/* #region Loaders */
fn process_loader(
    kind: &LoaderKind,
    attributes: Attributes,
    children: Children,
    parent: &BoxData,
) -> Option<WidgetData> {
    match kind {
        LoaderKind::Spinner => spinner(attributes),
        LoaderKind::LevelBar => level_bar(attributes),
        LoaderKind::ProgressBar => progress_bar(attributes, children, parent),
    }
}

fn spinner(attributes: Attributes) -> Option<WidgetData> {
    let mut data = SpinnerData::new();
    for attr in attributes {
        match attr.name() {
            "spin" | "spinning" => match attr.value() {
                "f" | "false" | "n" | "no" => data.spinning = false,
                _ => data.spinning = true,
            },
            _ => data.defaults.modify(attr),
        }
    }
    Some(WidgetData::Spinner(data))
}

#[derive(Debug, PartialEq, Clone)]
struct SpinnerData {
    defaults: WidgetDefaults,
    spinning: bool,
}

impl SpinnerData {
    pub fn new() -> Self {
        SpinnerData {
            defaults: WidgetDefaults::new(),
            spinning: true,
        }
    }
    pub fn build(&self) -> gtk4::Spinner {
        gtk4::Spinner::builder().spinning(self.spinning).build()
    }
}

fn level_bar(attributes: Attributes) -> Option<WidgetData> {
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
            "overflow" | "leak" | "of" => match val {
                "false" | "f" | "no" | "n" => data.of = gtk4::Overflow::Hidden,
                _ => data.of = gtk4::Overflow::Visible,
            },
            "invert" | "inverted" | "rev" | "reversed" => match val {
                "false" | "f" | "no" | "n" => data.inverted = false,
                _ => data.inverted = true,
            },
            _ => data.defaults.modify(attr),
        }
    }
    Some(WidgetData::LevelBar(data))
}

#[derive(Debug, PartialEq, Clone)]
struct LevelBarData {
    defaults: WidgetDefaults,
    progress: f64,
    min: f64,
    max: f64,
    mode: gtk4::LevelBarMode,
    of: gtk4::Overflow,
    inverted: bool,
}

impl LevelBarData {
    pub fn new() -> Self {
        LevelBarData {
            defaults: WidgetDefaults::new(),
            progress: 0f64,
            min: 0f64,
            max: 100f64,
            mode: gtk4::LevelBarMode::Continuous,
            of: gtk4::Overflow::Visible,
            inverted: false,
        }
    }
    pub fn build(&self) -> gtk4::LevelBar {
        gtk4::LevelBar::builder()
            .value(self.progress)
            .min_value(self.min)
            .max_value(self.max)
            .mode(self.mode)
            .overflow(self.of)
            .inverted(self.inverted)
            .build()
    }
}

#[derive(Debug, PartialEq, Clone)]
enum LoaderKind {
    Spinner,
    LevelBar,
    ProgressBar,
}

fn progress_bar(
    attributes: Attributes,
    children: Children,
    parent: &BoxData,
) -> Option<WidgetData> {
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
            _ => data.defaults.modify(attr),
        }
    }
    for child in children {
        if let Some(WidgetData::Label(label)) = process_element(&child, parent) {
            data.text.push_str(&label.text);
        }
    }
    Some(WidgetData::ProgressBar(data))
}

#[derive(Debug, PartialEq, Clone)]
struct ProgressBarData {
    defaults: WidgetDefaults,
    ellipsize: gtk4::pango::EllipsizeMode,
    progress: f64,
    inverted: bool,
    pulse: f64,
    text: String,
}

impl ProgressBarData {
    pub fn new() -> Self {
        ProgressBarData {
            defaults: WidgetDefaults::new(),
            ellipsize: gtk4::pango::EllipsizeMode::None,
            progress: 0f64,
            inverted: false,
            pulse: 0f64,
            text: String::new(),
        }
    }
    pub fn build(&self) -> gtk4::ProgressBar {
        gtk4::ProgressBar::builder()
            .fraction(self.progress)
            .inverted(self.inverted)
            .pulse_step(self.pulse)
            .text(&self.text)
            .ellipsize(self.ellipsize)
            .show_text(!self.text.is_empty())
            .build()
    }
}
/* #endregion Loaders */

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
    halign: gtk4::Align,
    valign: gtk4::Align,
    hexpand: bool,
    vexpand: bool,
    tooltip: Option<String>,
    opacity: f64,
    margin: Margin,
    name: String,
    size_req: Option<(i32, i32)>,
    orientation: gtk4::Orientation,
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
            size_req: None,
            orientation: gtk4::Orientation::Horizontal,
        }
    }
    pub fn modify(&mut self, attr: roxmltree::Attribute) {
        let val = attr.value();
        match attr.name() {
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
            _ => {}
        }
    }
    pub fn apply(&self, widget: &impl IsA<Widget>) {
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
        if let Some((width, height)) = &self.size_req {
            widget.set_size_request(*width, *height);
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ElemKind {
    Label(Text),
    Container(BoxKind),
    Button(ButtonKind),
    Loader(LoaderKind),
    Canvas(CanvasKind),
    Cloned,
    Fallback,
}

#[derive(Debug, PartialEq, Clone)]
enum WidgetData {
    Box(BoxData),
    Grid(GridData),
    Button(ButtonData),
    ToggleButton(ToggleButtonData),
    CheckButton(CheckButtonData),
    Label(Box<LabelData>),
    DrawingArea(DrawingAreaData),
    GLArea(GLAreaData),
    Clone(Box<WidgetData>),
    Spinner(SpinnerData),
    LevelBar(LevelBarData),
    ProgressBar(ProgressBarData),
}

impl WidgetData {
    pub fn build(&self, parent: &gtk4::Box) -> Widget {
        match self {
            WidgetData::Box(box_) => box_.build().into(),
            WidgetData::Grid(grid) => grid.build(parent).into(),
            WidgetData::Button(button) => button.build(parent).into(),
            WidgetData::ToggleButton(button) => button.build(parent).into(),
            WidgetData::CheckButton(button) => button.build(parent).into(),
            WidgetData::Label(label) => label.build(),
            WidgetData::DrawingArea(drawing) => drawing.build().into(),
            WidgetData::GLArea(glarea) => glarea.build().into(),
            WidgetData::Clone(clone) => clone.build(parent),
            WidgetData::Spinner(spinner) => spinner.build().into(),
            WidgetData::LevelBar(bar) => bar.build().into(),
            WidgetData::ProgressBar(bar) => bar.build().into(),
        }
    }
    pub fn get_name(&self) -> String {
        match self {
            WidgetData::Box(box_) => box_.defaults.name.to_string(),
            WidgetData::Grid(grid) => grid.defaults.name.to_string(),
            WidgetData::Button(button) => button.defaults.name.to_string(),
            WidgetData::ToggleButton(button) => button.defaults.name.to_string(),
            WidgetData::CheckButton(button) => button.defaults.name.to_string(),
            WidgetData::Label(label) => label.defaults.name.to_string(),
            WidgetData::DrawingArea(drawing) => drawing.defaults.name.to_string(),
            WidgetData::GLArea(glarea) => glarea.defaults.name.to_string(),
            WidgetData::Clone(clone) => clone.get_name(),
            WidgetData::Spinner(spinner) => spinner.defaults.name.to_string(),
            WidgetData::LevelBar(bar) => bar.defaults.name.to_string(),
            WidgetData::ProgressBar(bar) => bar.defaults.name.to_string(),
        }
    }
    pub fn set_name(&mut self, new_name: &str) {
        match self {
            WidgetData::Box(box_) => box_.defaults.name = new_name.to_string(),
            WidgetData::Grid(grid) => grid.defaults.name = new_name.to_string(),
            WidgetData::Button(button) => button.defaults.name = new_name.to_string(),
            WidgetData::ToggleButton(button) => button.defaults.name = new_name.to_string(),
            WidgetData::CheckButton(button) => button.defaults.name = new_name.to_string(),
            WidgetData::Label(label) => label.defaults.name = new_name.to_string(),
            WidgetData::DrawingArea(drawing) => drawing.defaults.name = new_name.to_string(),
            WidgetData::GLArea(glarea) => glarea.defaults.name = new_name.to_string(),
            WidgetData::Clone(clone) => clone.set_name(new_name),
            WidgetData::Spinner(spinner) => spinner.defaults.name = new_name.to_string(),
            WidgetData::LevelBar(bar) => bar.defaults.name = new_name.to_string(),
            WidgetData::ProgressBar(bar) => bar.defaults.name = new_name.to_string(),
        }
    }
}
