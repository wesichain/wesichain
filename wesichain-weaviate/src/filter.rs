use wesichain_core::{MetadataFilter, Value};

use crate::WeaviateStoreError;

pub fn to_weaviate_filter(filter: &MetadataFilter) -> Result<String, WeaviateStoreError> {
    filter_to_where(filter)
}

fn filter_to_where(filter: &MetadataFilter) -> Result<String, WeaviateStoreError> {
    match filter {
        MetadataFilter::Eq(key, value) => {
            let path = graphql_path(path_segments(key)?);
            let value = eq_value_clause(key, value)?;
            Ok(format!("{{operator:Equal,path:{path},{value}}}"))
        }
        MetadataFilter::In(key, values) => {
            let path = graphql_path(path_segments(key)?);
            let value = in_value_clause(key, values)?;
            Ok(format!("{{operator:ContainsAny,path:{path},{value}}}"))
        }
        MetadataFilter::Range { key, min, max } => range_clause(key, min.as_ref(), max.as_ref()),
        MetadataFilter::All(filters) => logical_clause("And", "all", filters),
        MetadataFilter::Any(filters) => logical_clause("Or", "any", filters),
    }
}

fn logical_clause(
    op: &str,
    key: &str,
    filters: &[MetadataFilter],
) -> Result<String, WeaviateStoreError> {
    if filters.is_empty() {
        return Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: format!("{key}(...) must not be empty"),
        });
    }

    let mut operands = Vec::with_capacity(filters.len());
    for filter in filters {
        operands.push(filter_to_where(filter)?);
    }

    Ok(format!(
        "{{operator:{op},operands:[{}]}}",
        operands.join(",")
    ))
}

fn range_clause(
    key: &str,
    min: Option<&Value>,
    max: Option<&Value>,
) -> Result<String, WeaviateStoreError> {
    let path = graphql_path(path_segments(key)?);
    let mut operands = Vec::with_capacity(2);

    if let Some(min) = min {
        let min = numeric_value(key, "min", min)?;
        operands.push(format!(
            "{{operator:GreaterThanEqual,path:{path},valueNumber:{min}}}"
        ));
    }

    if let Some(max) = max {
        let max = numeric_value(key, "max", max)?;
        operands.push(format!(
            "{{operator:LessThanEqual,path:{path},valueNumber:{max}}}"
        ));
    }

    match operands.len() {
        0 => Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "range requires at least one numeric bound".to_string(),
        }),
        1 => Ok(operands.into_iter().next().expect("single operand exists")),
        _ => Ok(format!(
            "{{operator:And,operands:[{}]}}",
            operands.join(",")
        )),
    }
}

fn path_segments(key: &str) -> Result<Vec<&str>, WeaviateStoreError> {
    if key.is_empty() {
        return Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "metadata key must not be empty".to_string(),
        });
    }

    let segments = key.split('.').collect::<Vec<_>>();

    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "metadata key must not contain empty path segments".to_string(),
        });
    }

    Ok(segments)
}

fn graphql_path(segments: Vec<&str>) -> String {
    let encoded = segments
        .into_iter()
        .map(graphql_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{encoded}]")
}

fn eq_value_clause(key: &str, value: &Value) -> Result<String, WeaviateStoreError> {
    match value {
        Value::String(value) => Ok(format!("valueText:{}", graphql_string(value))),
        Value::Bool(value) => Ok(format!("valueBoolean:{value}")),
        Value::Number(_) => Ok(format!("valueNumber:{}", numeric_value(key, "eq", value)?)),
        Value::Null => Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "null equality is not supported by weaviate filters".to_string(),
        }),
        Value::Array(_) => Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "array equality is not supported by weaviate filters".to_string(),
        }),
        Value::Object(_) => Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "object equality is not supported by weaviate filters".to_string(),
        }),
    }
}

fn in_value_clause(key: &str, values: &[Value]) -> Result<String, WeaviateStoreError> {
    if values.is_empty() {
        return Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "in(...) requires at least one value".to_string(),
        });
    }

    if values.iter().all(Value::is_string) {
        let values = values
            .iter()
            .map(|value| graphql_string(value.as_str().expect("validated as string")))
            .collect::<Vec<_>>()
            .join(",");
        return Ok(format!("valueTextArray:[{values}]"));
    }

    if values.iter().all(Value::is_boolean) {
        let values = values
            .iter()
            .map(|value| value.as_bool().expect("validated as bool").to_string())
            .collect::<Vec<_>>()
            .join(",");
        return Ok(format!("valueBooleanArray:[{values}]"));
    }

    if values.iter().all(Value::is_number) {
        let values = values
            .iter()
            .map(|value| numeric_value(key, "in", value))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        return Ok(format!("valueNumberArray:[{values}]"));
    }

    Err(WeaviateStoreError::UnsupportedFilterValue {
        key: key.to_string(),
        reason:
            "in(...) supports only homogeneous string, bool, or number values in weaviate filters"
                .to_string(),
    })
}

fn numeric_value(key: &str, op: &str, value: &Value) -> Result<String, WeaviateStoreError> {
    let number = value
        .as_f64()
        .ok_or_else(|| WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: format!("{op} bound/value must be a finite number"),
        })?;

    if number.is_finite() {
        Ok(value.to_string())
    } else {
        Err(WeaviateStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: format!("{op} bound/value must be a finite number"),
        })
    }
}

fn graphql_string(value: &str) -> String {
    serde_json::to_string(value).expect("string escaping should not fail")
}
