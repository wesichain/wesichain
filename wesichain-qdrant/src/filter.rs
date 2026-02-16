use qdrant_client::qdrant::{
    condition, r#match, Condition, FieldCondition, Filter, Match, Range, RepeatedIntegers,
    RepeatedStrings,
};
use serde_json::{Map as JsonMap, Value};
use wesichain_core::MetadataFilter;

use crate::QdrantStoreError;

pub fn to_qdrant_filter(filter: &MetadataFilter) -> Result<Filter, QdrantStoreError> {
    filter_to_filter(filter)
}

pub fn qdrant_filter_to_payload(filter: &Filter) -> Result<Value, QdrantStoreError> {
    filter_payload(filter)
}

fn filter_to_filter(filter: &MetadataFilter) -> Result<Filter, QdrantStoreError> {
    match filter {
        MetadataFilter::Eq(key, value) => Ok(Filter {
            must: vec![eq_condition(key, value)?],
            ..Filter::default()
        }),
        MetadataFilter::In(key, values) => Ok(Filter {
            must: vec![in_condition(key, values)?],
            ..Filter::default()
        }),
        MetadataFilter::Range { key, min, max } => Ok(Filter {
            must: vec![range_condition(key, min.as_ref(), max.as_ref())?],
            ..Filter::default()
        }),
        MetadataFilter::All(filters) => {
            if filters.is_empty() {
                return Err(QdrantStoreError::UnsupportedFilterValue {
                    key: "all".to_string(),
                    reason: "all(...) must not be empty".to_string(),
                });
            }

            let mut must = Vec::with_capacity(filters.len());
            for nested in filters {
                must.push(filter_condition(nested)?);
            }

            Ok(Filter {
                must,
                ..Filter::default()
            })
        }
        MetadataFilter::Any(filters) => {
            if filters.is_empty() {
                return Err(QdrantStoreError::UnsupportedFilterValue {
                    key: "any".to_string(),
                    reason: "any(...) must not be empty".to_string(),
                });
            }

            let mut should = Vec::with_capacity(filters.len());
            for nested in filters {
                should.push(filter_condition(nested)?);
            }

            Ok(Filter {
                should,
                ..Filter::default()
            })
        }
    }
}

fn filter_condition(filter: &MetadataFilter) -> Result<Condition, QdrantStoreError> {
    Ok(Condition {
        condition_one_of: Some(condition::ConditionOneOf::Filter(filter_to_filter(filter)?)),
    })
}

fn eq_condition(key: &str, value: &Value) -> Result<Condition, QdrantStoreError> {
    let field = match value {
        Value::Bool(value) => FieldCondition {
            key: key.to_string(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Boolean(*value)),
            }),
            ..FieldCondition::default()
        },
        Value::String(value) => FieldCondition {
            key: key.to_string(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(value.clone())),
            }),
            ..FieldCondition::default()
        },
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                FieldCondition {
                    key: key.to_string(),
                    r#match: Some(Match {
                        match_value: Some(r#match::MatchValue::Integer(value)),
                    }),
                    ..FieldCondition::default()
                }
            } else if let Some(value) = number.as_u64() {
                let value = u64_to_i64(key, "eq", value)?;
                FieldCondition {
                    key: key.to_string(),
                    r#match: Some(Match {
                        match_value: Some(r#match::MatchValue::Integer(value)),
                    }),
                    ..FieldCondition::default()
                }
            } else {
                FieldCondition {
                    key: key.to_string(),
                    range: Some(number_eq_range(key, value)?),
                    ..FieldCondition::default()
                }
            }
        }
        Value::Null => {
            return Err(QdrantStoreError::UnsupportedFilterValue {
                key: key.to_string(),
                reason: "null is not supported by Qdrant payload filters".to_string(),
            });
        }
        Value::Array(_) => {
            return Err(QdrantStoreError::UnsupportedFilterValue {
                key: key.to_string(),
                reason: "array equality is not supported by Qdrant payload filters".to_string(),
            });
        }
        Value::Object(_) => {
            return Err(QdrantStoreError::UnsupportedFilterValue {
                key: key.to_string(),
                reason: "object equality is not supported by Qdrant payload filters".to_string(),
            });
        }
    };

    Ok(Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(field)),
    })
}

fn in_condition(key: &str, values: &[Value]) -> Result<Condition, QdrantStoreError> {
    if values.is_empty() {
        return Err(QdrantStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "in(...) requires at least one value".to_string(),
        });
    }

    let field = if values.iter().all(Value::is_string) {
        let values = values
            .iter()
            .map(|value| value.as_str().expect("validated as string").to_string())
            .collect::<Vec<_>>();

        FieldCondition {
            key: key.to_string(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keywords(RepeatedStrings {
                    strings: values,
                })),
            }),
            ..FieldCondition::default()
        }
    } else if values
        .iter()
        .all(|value| value.is_i64() || value.as_u64().is_some())
    {
        let values = values
            .iter()
            .map(|value| {
                if let Some(value) = value.as_i64() {
                    Ok(value)
                } else {
                    let value = value.as_u64().expect("validated as u64");
                    u64_to_i64(key, "in", value)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        FieldCondition {
            key: key.to_string(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Integers(RepeatedIntegers {
                    integers: values,
                })),
            }),
            ..FieldCondition::default()
        }
    } else {
        return Err(QdrantStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason:
                "in(...) supports only homogeneous string or int64 values in Qdrant payload filters"
                    .to_string(),
        });
    };

    Ok(Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(field)),
    })
}

fn u64_to_i64(key: &str, op: &str, value: u64) -> Result<i64, QdrantStoreError> {
    i64::try_from(value).map_err(|_| QdrantStoreError::UnsupportedFilterValue {
        key: key.to_string(),
        reason: format!("{op} value must fit int64; u64 value exceeds i64::MAX: {value}"),
    })
}

fn range_condition(
    key: &str,
    min: Option<&Value>,
    max: Option<&Value>,
) -> Result<Condition, QdrantStoreError> {
    let gte = match min {
        Some(value) => Some(json_number_to_f64(key, "min", value)?),
        None => None,
    };
    let lte = match max {
        Some(value) => Some(json_number_to_f64(key, "max", value)?),
        None => None,
    };

    if gte.is_none() && lte.is_none() {
        return Err(QdrantStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: "range requires at least one numeric bound".to_string(),
        });
    }

    let field = FieldCondition {
        key: key.to_string(),
        range: Some(Range {
            gte,
            lte,
            ..Range::default()
        }),
        ..FieldCondition::default()
    };

    Ok(Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(field)),
    })
}

fn number_eq_range(key: &str, value: &Value) -> Result<Range, QdrantStoreError> {
    let eq = json_number_to_f64(key, "eq", value)?;
    Ok(Range {
        gte: Some(eq),
        lte: Some(eq),
        ..Range::default()
    })
}

fn json_number_to_f64(key: &str, op: &str, value: &Value) -> Result<f64, QdrantStoreError> {
    let number = value
        .as_f64()
        .ok_or_else(|| QdrantStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: format!("{op} bound/value must be a finite number"),
        })?;

    if number.is_finite() {
        Ok(number)
    } else {
        Err(QdrantStoreError::UnsupportedFilterValue {
            key: key.to_string(),
            reason: format!("{op} bound/value must be a finite number"),
        })
    }
}

fn filter_payload(filter: &Filter) -> Result<Value, QdrantStoreError> {
    let mut out = JsonMap::new();

    if !filter.must.is_empty() {
        let must = filter
            .must
            .iter()
            .map(condition_payload)
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("must".to_string(), Value::Array(must));
    }

    if !filter.should.is_empty() {
        let should = filter
            .should
            .iter()
            .map(condition_payload)
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("should".to_string(), Value::Array(should));
    }

    if !filter.must_not.is_empty() {
        let must_not = filter
            .must_not
            .iter()
            .map(condition_payload)
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("must_not".to_string(), Value::Array(must_not));
    }

    Ok(Value::Object(out))
}

fn condition_payload(condition: &Condition) -> Result<Value, QdrantStoreError> {
    match condition.condition_one_of.as_ref() {
        Some(condition::ConditionOneOf::Field(field)) => field_payload(field),
        Some(condition::ConditionOneOf::Filter(filter)) => filter_payload(filter),
        _ => Err(QdrantStoreError::UnsupportedFilterValue {
            key: "<condition>".to_string(),
            reason: "unsupported qdrant condition generated".to_string(),
        }),
    }
}

fn field_payload(field: &FieldCondition) -> Result<Value, QdrantStoreError> {
    let mut out = JsonMap::new();
    out.insert("key".to_string(), Value::String(field.key.clone()));

    if let Some(r#match) = field.r#match.as_ref() {
        out.insert("match".to_string(), match_payload(r#match)?);
    }

    if let Some(range) = field.range.as_ref() {
        let mut range_payload = JsonMap::new();
        if let Some(gt) = range.gt {
            range_payload.insert("gt".to_string(), Value::from(gt));
        }
        if let Some(gte) = range.gte {
            range_payload.insert("gte".to_string(), Value::from(gte));
        }
        if let Some(lt) = range.lt {
            range_payload.insert("lt".to_string(), Value::from(lt));
        }
        if let Some(lte) = range.lte {
            range_payload.insert("lte".to_string(), Value::from(lte));
        }

        out.insert("range".to_string(), Value::Object(range_payload));
    }

    Ok(Value::Object(out))
}

fn match_payload(r#match: &Match) -> Result<Value, QdrantStoreError> {
    let mut out = JsonMap::new();

    match r#match.match_value.as_ref() {
        Some(r#match::MatchValue::Keyword(value)) => {
            out.insert("value".to_string(), Value::String(value.clone()));
        }
        Some(r#match::MatchValue::Integer(value)) => {
            out.insert("value".to_string(), Value::from(*value));
        }
        Some(r#match::MatchValue::Boolean(value)) => {
            out.insert("value".to_string(), Value::Bool(*value));
        }
        Some(r#match::MatchValue::Keywords(values)) => {
            out.insert(
                "any".to_string(),
                Value::Array(
                    values
                        .strings
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect::<Vec<_>>(),
                ),
            );
        }
        Some(r#match::MatchValue::Integers(values)) => {
            out.insert(
                "any".to_string(),
                Value::Array(
                    values
                        .integers
                        .iter()
                        .copied()
                        .map(Value::from)
                        .collect::<Vec<_>>(),
                ),
            );
        }
        _ => {
            return Err(QdrantStoreError::UnsupportedFilterValue {
                key: "<match>".to_string(),
                reason: "unsupported qdrant match variant generated".to_string(),
            });
        }
    }

    Ok(Value::Object(out))
}
