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
    let mut depth = 0_i32;
    let mut datatype = 0u8;
    let mut escaping = false;
    let mut tailing = false;
    let mut result = Vec::new();
    let mut head = String::new();
    let mut subdata = String::new();
    for char in data.chars() {
        let mut subescaping = false;
        subdata.push(char);
        if !char.is_numeric() && datatype == 3 {
            datatype = 0;
            depth -= 1;
            if depth == 0 {
                subdata.pop();
                tailing = false;
                result.push((head.trim().to_string(), subdata.trim().to_string()));
                println!("Head: {}, Tail: {}", head.trim(), subdata.trim());
                subdata = String::new();
                head = String::new();
            } else if depth < 0 {
                return Vec::new();
            }
        }
        match char {
            '{' | ':' | '=' => {
                if datatype == 0 && !escaping {
                    if !tailing {
                        subdata.pop();
                        head = subdata.trim().to_string();
                        subdata = String::new();
                    }
                    tailing = true;
                    depth += 1;
                }
            }
            '}' => {
                if datatype == 0 && !escaping {
                    depth -= 1;
                    if depth == 0 {
                        subdata.pop();
                        tailing = false;
                        result.push((head.trim().to_string(), subdata.trim().to_string()));
                        println!("Head: {}, Tail: {}", head.trim(), subdata.trim());
                        subdata = String::new();
                        head = String::new();
                    } else if depth < 0 {
                        return Vec::new();
                    }
                }
            }
            '\\' => {
                if datatype != 0 && !escaping && depth < 2 {
                    subescaping = true;
                    subdata.pop();
                }
            }
            '\'' | '"' => {
                if !tailing {
                    return Vec::new();
                }
                if !escaping {
                    if datatype == 0 {
                        depth += 1;
                        datatype = if char == '\'' { 1 } else { 2 };
                    } else if datatype == if char == '\'' { 1 } else { 2 } {
                        datatype = 0;
                        depth -= 1;
                        if depth == 0 {
                            tailing = false;
                            result.push((head.trim().to_string(), subdata.trim().to_string()));
                            println!("Head: {}, Tail: {}", head.trim(), subdata.trim());
                            subdata = String::new();
                            head = String::new();
                        } else if depth < 0 {
                            return Vec::new();
                        }
                    }
                }
            }
            ';' | ',' => {
                if !escaping && datatype == 0 {
                    subdata.pop();
                }
            }
            '0'..='9' => {
                if !escaping && datatype == 0 {
                    datatype = 3;
                }
            }
            _ => {}
        }
        escaping = subescaping;
    }
    result
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
