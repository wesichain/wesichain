use serde::{de::DeserializeOwned, Serialize};

use crate::WesichainError;

pub type Value = serde_json::Value;

pub trait IntoValue {
    fn into_value(self) -> Value;
}

pub trait TryFromValue: Sized {
    fn try_from_value(value: Value) -> Result<Self, WesichainError>;
}

impl<T> IntoValue for T
where
    T: Serialize,
{
    fn into_value(self) -> Value {
        serde_json::to_value(self).expect("Failed to serialize into wesichain_core::Value")
    }
}

impl<T> TryFromValue for T
where
    T: DeserializeOwned,
{
    fn try_from_value(value: Value) -> Result<Self, WesichainError> {
        Ok(serde_json::from_value(value)?)
    }
}
