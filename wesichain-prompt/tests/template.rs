use std::collections::HashMap;

use wesichain_core::Value;
use wesichain_prompt::PromptTemplate;

#[test]
fn renders_template_with_vars() {
    let tmpl = PromptTemplate::new("Hello {{name}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::from("Wesi"));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "Hello Wesi");
}

#[test]
fn does_not_confuse_overlapping_keys() {
    let tmpl = PromptTemplate::new("{{name}} {{fullname}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::from("X"));
    vars.insert("fullname".to_string(), Value::from("Y"));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "X Y");
}

#[test]
fn missing_var_kept() {
    let tmpl = PromptTemplate::new("Hi {{name}}".to_string());
    let vars = HashMap::new();
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "Hi {{name}}");
}

#[test]
fn renders_key_with_dot() {
    let tmpl = PromptTemplate::new("User {{user.name}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("user.name".to_string(), Value::from("Ana"));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "User Ana");
}

#[test]
fn renders_non_string_value() {
    let tmpl = PromptTemplate::new("Count {{total}}".to_string());
    let mut vars = HashMap::new();
    vars.insert("total".to_string(), Value::from(42));
    let rendered = tmpl.render(&vars).expect("render");
    assert_eq!(rendered, "Count 42");
}
