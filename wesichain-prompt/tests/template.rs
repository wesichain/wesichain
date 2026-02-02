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
