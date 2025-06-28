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

fn build_ui(app: &Application, elements: &[(String, String)]) {
    let widgets = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    for element in elements {
        match element.0.as_str() {
            "button" => {
                let settings = get_elements(&element.1);
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
            "text" => {
                break;
            }
            _ => {}
        }
    }
    let window = ApplicationWindow::builder()
        .application(app)
        .title(stringify("title", elements, "Dither Browser"))
        .child(&widgets)
        .build();
    window.present();
}

fn get_elements(data: &str) -> Vec<(String, String)> {
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
                        output.push((header.clone(), block.trim().to_string()));
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
                                    output.push((header.clone(), block.trim().to_string()));
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
    if deftype == 3 {
        output.push((header.clone(), block.trim().to_string()));
    }
    output
}

fn stringify(key: &str, array: &[(String, String)], fallback: &str) -> String {
    let input = array
        .iter()
        .find(|&(skey, _)| skey == key)
        .map(|(_, value)| value.clone());
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

fn numerify(key: &str, array: &[(String, String)], fallback: i32) -> i32 {
    let input = array
        .iter()
        .find(|&(skey, _)| skey == key)
        .map(|(_, value)| value.clone());
    match input {
        None => fallback,
        Some(x) => x.parse().unwrap_or(fallback),
    }
}
