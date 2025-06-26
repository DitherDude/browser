use gtk::{Application, glib};
use gtk::{ApplicationWindow, prelude::*};
use regex::Regex;
use std::collections::HashMap;
use std::fs;

const APP_ID: &str = "dither.browser";

fn main() -> glib::ExitCode {
    let data = fs::read_to_string("input.dd").unwrap();
    let elements = get_elements(&data);
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, &elements));
    app.run()
}

fn build_ui(app: &Application, elements: &HashMap<String, String>) {
    let widgets = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    for element in elements {
        match element.0.as_str() {
            "button" => {
                let settings = get_elements(element.1);
                let label = stringify(settings.get("label")).unwrap_or("Edit Me!".to_string());
                let margin_top = numerify(settings.get("margin_top")).unwrap_or(12);
                let margin_bottom = numerify(settings.get("margin_bottom")).unwrap_or(12);
                let margin_start = numerify(settings.get("margin_start")).unwrap_or(12);
                let margin_end = numerify(settings.get("margin_end")).unwrap_or(12);
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
        .title("Some program ig lolz")
        .child(&widgets)
        .build();
    window.present();
}

fn get_elements(data: &str) -> HashMap<String, String> {
    let re = Regex::new(r"\s*").unwrap();
    let mut pushval = 0_i32;
    let mut instring = None;
    let mut escaping = false;
    let mut value = false;
    let mut result = HashMap::new();
    let mut element = (String::new(), String::new());
    let mut subdata = String::new();
    for char in data.chars() {
        subdata.push(char);
        let mut subescaping = false;
        match char {
            '{' => {
                if !(instring.is_some() || escaping) {
                    if !value {
                        subdata.pop();
                        subdata = re.replace(&subdata, "").to_string();
                        element.0 = subdata;
                        subdata = String::new();
                    }
                    value = true;
                    pushval += 1;
                }
            }
            '}' => {
                if !(instring.is_some() || escaping) {
                    pushval -= 1;
                    if pushval == 0 {
                        subdata.pop();
                        element.1 = subdata;
                        subdata = String::new();
                        value = false;
                        result.insert(element.0.clone(), element.1.clone());
                        element = (String::new(), String::new());
                    } else if pushval < 0 {
                        return HashMap::new();
                    }
                }
            }
            '\\' => {
                if instring.is_some() && !escaping {
                    subescaping = true;
                }
            }
            '\'' | '"' => {
                if !value {
                    return HashMap::new();
                }
                if !escaping {
                    if instring.is_none() {
                        instring = Some(char);
                    } else if instring == Some(char) {
                        instring = None;
                    }
                }
            }
            _ => {}
        }
        escaping = subescaping;
    }
    result
}

fn stringify(input: Option<&String>) -> Option<String> {
    match input {
        None => None,
        Some(x) => {
            if (x.starts_with('\"') && x.ends_with('\"'))
                || (x.starts_with('\'') && x.ends_with('\''))
            {
                Some(x[1..x.len() - 1].to_string())
            } else {
                None
            }
        }
    }
}

fn numerify(input: Option<&String>) -> Option<i32> {
    match input {
        None => None,
        Some(x) => x.parse().ok(),
    }
}
