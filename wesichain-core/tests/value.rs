use serde::{Deserialize, Serialize};
use wesichain_core::{IntoValue, TryFromValue, Value};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Demo {
    name: String,
}

#[test]
fn value_roundtrip_for_struct() {
    let input = Demo {
        name: "alpha".to_string(),
    };
    let value: Value = input.into_value();
    let output = Demo::try_from_value(value).expect("convert back");
    assert_eq!(
        output,
        Demo {
            name: "alpha".to_string(),
        }
    );
}
