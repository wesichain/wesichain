use serde::{Deserialize, Serialize};
use wesichain_core::{IntoValue, TryFromValue, Value};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Demo {
    name: String,
}

struct FailingSerialize;

impl Serialize for FailingSerialize {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom(
            "serialize fails for IntoValue null fallback",
        ))
    }
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

#[test]
fn into_value_falls_back_to_null_on_serialize_error() {
    let value: Value = FailingSerialize.into_value();
    assert_eq!(value, Value::Null);
}
