use gtk::{Application, glib};
use gtk::{ApplicationWindow, prelude::*};
use std::fs;

const APP_ID: &str = "dither.browser";

fn main() -> glib::ExitCode {
    let data = fs::read_to_string("input.dd").unwrap();
    let elements = get_elements(&data);
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, &elements));
    app.run()
}

fn build_ui(app: &Application, elements: &[Dictionary]) {
    let widgets = glib_box(elements);
    let information = get_elements(&getrawcontents("data", elements, ""));
    let window = ApplicationWindow::builder()
        .application(app)
        .title(stringify("title", &information, "Dither Browser"))
        .child(&widgets)
        .build();
    window.present();
}

fn glib_box(elements: &[Dictionary]) -> gtk::Box {
    let widgets = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    for element in elements {
        match element.key.as_str() {
            "data" => {
                break;
            }
            "button" => {
                let settings = get_elements(&element.value);
                let label = stringify("label", &settings, "Edit Me!");
                let margin_top = numerify("margin_top", &settings, 12);
                let margin_bottom = numerify("margin_bottom", &settings, 12);
                let margin_start = numerify("margin_start", &settings, 12);
                let margin_end = numerify("margin_end", &settings, 12);
                let button = gtk::Button::builder()
                    .label(label)
                    .margin_top(margin_top)
                    .margin_bottom(margin_bottom)
                    .margin_start(margin_start)
                    .margin_end(margin_end)
                    .build();
                widgets.append(&button);
            }
            "box" => {
                let settings = get_elements(&element.value);
                let margin_top = numerify("margin_top", &settings, 12);
                let margin_bottom = numerify("margin_bottom", &settings, 12);
                let margin_start = numerify("margin_start", &settings, 12);
                let margin_end = numerify("margin_end", &settings, 12);
                let items = glib_box(&get_elements(&getrawcontents("data", &settings, "")));
                let gtk_box = gtk::Box::builder()
                    .margin_top(margin_top)
                    .margin_bottom(margin_bottom)
                    .margin_start(margin_start)
                    .margin_end(margin_end)
                    .build();
                gtk_box.append(&items);
                widgets.append(&gtk_box);
            }
            _ => {}
        }
    }
    widgets
}

fn get_elements(data: &str) -> Vec<Dictionary> {
    let mut output = Vec::new();
    let mut depth = 0_i32;
    let mut header = String::new();
    let mut block = String::new();
    let mut instring = None;
    let mut escaping = false;
    let mut deftype = 0u8;
    for char in data.chars() {
        if depth < 0 {
            break;
        }
        match char {
            '{' => {
                if instring.is_none() {
                    depth += 1;
                    if depth == 1 {
                        header = block.trim().to_string();
                        block = String::new();
                        continue;
                    }
                }
            }
            '=' | ':' => {
                if instring.is_none() && depth == 0 {
                    deftype = 1;
                    depth += 1;
                    if depth == 1 {
                        header = block.trim().to_string();
                        block = String::new();
                        continue;
                    }
                }
            }
            '}' => {
                if instring.is_none() {
                    depth -= 1;
                    if depth == 0 {
                        output.push(Dictionary::new(header.clone(), block.trim().to_string()));
                        block = String::new();
                        continue;
                    }
                }
            }
            x => {
                if depth == 1 {
                    match x {
                        '\'' | '"' => {
                            if instring.is_none() {
                                instring = Some(x);
                            } else if instring == Some(x) && !escaping {
                                instring = None;
                                if deftype == 1 {
                                    deftype = 2;
                                }
                            }
                        }
                        '\\' => {
                            if instring.is_some() && !escaping {
                                escaping = true;
                                continue;
                            }
                        }
                        ' ' | '\n' => {
                            if instring.is_none() && deftype >= 2 {
                                deftype = 0;
                                depth -= 1;
                                if depth == 0 {
                                    output.push(Dictionary::new(
                                        header.clone(),
                                        block.trim().to_string(),
                                    ));
                                    block = String::new();
                                    continue;
                                }
                            }
                        }
                        '0'..='9' => {
                            if instring.is_none() && deftype == 1 {
                                deftype = 3;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        block.push(char);
        escaping = false;
    }
    if deftype > 1 {
        output.push(Dictionary::new(header.clone(), block.trim().to_string()));
    }
    println!("Output: {:?}", output);
    output
}

fn stringify(key: &str, array: &[Dictionary], fallback: &str) -> String {
    let input = array
        .iter()
        .find(|dict| dict.key == key)
        .map(|dict| dict.value.clone());
    match input {
        None => fallback.to_string(),
        Some(x) => {
            if (x.starts_with('\"') && x.ends_with('\"'))
                || (x.starts_with('\'') && x.ends_with('\''))
            {
                x[1..x.len() - 1].to_string()
            } else {
                fallback.to_string()
            }
        }
    }
}

fn numerify(key: &str, array: &[Dictionary], fallback: i32) -> i32 {
    let input = array
        .iter()
        .find(|dict| dict.key == key)
        .map(|dict| dict.value.clone());
    match input {
        None => fallback,
        Some(x) => x.parse().unwrap_or(fallback),
    }
}

fn getrawcontents(key: &str, array: &[Dictionary], fallback: &str) -> String {
    let input = array
        .iter()
        .find(|dict| dict.key == key)
        .map(|dict| dict.value.clone());
    match input {
        None => fallback.to_string(),
        Some(x) => x,
    }
}

#[derive(Debug)]
struct Dictionary {
    key: String,
    value: String,
}

impl Dictionary {
    fn new(key: String, value: String) -> Dictionary {
        Dictionary { key, value }
    }
}
