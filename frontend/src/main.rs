use regex::Regex;
use std::fs;

fn main() {
    let data = fs::read_to_string("input.dd").unwrap();
    let elements = get_elements(&data);
    for element in elements {
        parse_element(&element);
    }
}

fn get_elements(rdata: &str) -> Vec<PageElement> {
    let re = Regex::new(r"\s+").unwrap();
    let data = re.replace_all(rdata, " ");
    let mut pushval = 0_i32;
    let mut instring = None;
    let mut escaping = false;
    let mut value = false;
    let mut result = Vec::new();
    let mut element = PageElement::new();
    let mut subdata = String::new();
    for char in data.chars() {
        subdata.push(char);
        let mut subescaping = false;
        match char {
            '{' => {
                if !(instring.is_some() || escaping) {
                    if !value {
                        subdata.pop();
                        element.name = subdata;
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
                        element.value = subdata;
                        subdata = String::new();
                        value = false;
                        result.push(element);
                        element = PageElement::new();
                    } else if pushval < 0 {
                        return Vec::new();
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
                    return Vec::new();
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

fn parse_element(element: &PageElement) {
    match element.name.as_str() {
        "slider" => {
            let _options = get_elements(&element.value);
        }
        "box" => {
            let _options = get_elements(&element.value);
        }
        _ => {}
    }
}

#[derive(Debug)]
struct PageElement {
    name: String,
    value: String,
}

impl PageElement {
    fn new() -> PageElement {
        PageElement {
            name: String::new(),
            value: String::new(),
        }
    }
}
