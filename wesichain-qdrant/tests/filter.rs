use qdrant_client::qdrant::{condition, r#match, Condition};
use serde_json::json;
use wesichain_core::MetadataFilter;
use wesichain_qdrant::{
    filter::{qdrant_filter_to_payload, to_qdrant_filter},
    QdrantStoreError,
};

#[test]
fn converts_eq_filter() {
    let filter = MetadataFilter::Eq("source".to_string(), json!("tweet"));

    let out = to_qdrant_filter(&filter).expect("eq filter should convert");
    assert_eq!(out.must.len(), 1);

    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            assert_eq!(field.key, "source");
            let r#match = field.r#match.expect("eq should map to match");
            assert!(matches!(
                r#match.match_value,
                Some(r#match::MatchValue::Keyword(_))
            ));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn converts_in_filter() {
    let filter = MetadataFilter::In("lang".to_string(), vec![json!("en"), json!("id")]);

    let out = to_qdrant_filter(&filter).expect("in filter should convert");
    assert_eq!(out.must.len(), 1);

    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            assert_eq!(field.key, "lang");
            let r#match = field.r#match.expect("in should map to match any");
            assert!(matches!(
                r#match.match_value,
                Some(r#match::MatchValue::Keywords(_))
            ));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn converts_range_filter() {
    let filter = MetadataFilter::Range {
        key: "score".to_string(),
        min: Some(json!(0.25)),
        max: Some(json!(0.75)),
    };

    let out = to_qdrant_filter(&filter).expect("range filter should convert");
    assert_eq!(out.must.len(), 1);

    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            assert_eq!(field.key, "score");
            let range = field.range.expect("range should map to range payload");
            assert_eq!(range.gte, Some(0.25));
            assert_eq!(range.lte, Some(0.75));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn converts_nested_all_any_filter() {
    let filter = MetadataFilter::All(vec![
        MetadataFilter::Eq("source".to_string(), json!("guide")),
        MetadataFilter::Any(vec![
            MetadataFilter::In("lang".to_string(), vec![json!("en"), json!("de")]),
            MetadataFilter::Range {
                key: "score".to_string(),
                min: Some(json!(0.9)),
                max: None,
            },
        ]),
    ]);

    let out = to_qdrant_filter(&filter).expect("nested all/any filter should convert");
    assert_eq!(out.must.len(), 2);

    let nested = out.must[1].clone();
    match nested.condition_one_of {
        Some(condition::ConditionOneOf::Filter(inner)) => {
            assert_eq!(inner.should.len(), 2);
            assert!(inner.must.is_empty());
        }
        other => panic!("unexpected nested variant: {other:?}"),
    }
}

#[test]
fn converts_eq_i64_to_integer_match_without_range() {
    let filter = MetadataFilter::Eq("big_int".to_string(), json!(9_007_199_254_740_993_i64));

    let out = to_qdrant_filter(&filter).expect("eq i64 filter should convert");
    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            assert!(field.range.is_none(), "eq integer must not use range");
            let r#match = field.r#match.expect("eq integer should map to match");
            assert!(matches!(
                r#match.match_value,
                Some(r#match::MatchValue::Integer(9_007_199_254_740_993))
            ));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn converts_eq_f64_to_range() {
    let filter = MetadataFilter::Eq("score".to_string(), json!(0.125));

    let out = to_qdrant_filter(&filter).expect("eq float filter should convert");
    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            assert!(
                field.r#match.is_none(),
                "eq float must not use integer match"
            );
            let range = field.range.expect("eq float should map to range");
            assert_eq!(range.gte, Some(0.125));
            assert_eq!(range.lte, Some(0.125));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn rejects_eq_u64_value_above_i64_max() {
    let filter = MetadataFilter::Eq("count".to_string(), json!(9_223_372_036_854_775_808_u64));

    let err = to_qdrant_filter(&filter).expect_err("overflowing eq u64 should fail");

    assert!(matches!(
        err,
        QdrantStoreError::UnsupportedFilterValue { ref key, ref reason }
            if key == "count" && reason.contains("u64 value exceeds i64::MAX")
    ));
}

#[test]
fn converts_in_u64_values_that_fit_i64() {
    let filter = MetadataFilter::In(
        "count".to_string(),
        vec![json!(1_u64), json!(2_u64), json!(i64::MAX as u64)],
    );

    let out = to_qdrant_filter(&filter).expect("in u64 filter should convert");
    let Condition { condition_one_of } = out.must[0].clone();

    match condition_one_of {
        Some(condition::ConditionOneOf::Field(field)) => {
            let r#match = field.r#match.expect("in integers should map to match any");
            assert!(matches!(
                r#match.match_value,
                Some(r#match::MatchValue::Integers(ref values))
                    if values.integers == vec![1_i64, 2_i64, i64::MAX]
            ));
        }
        other => panic!("unexpected condition variant: {other:?}"),
    }
}

#[test]
fn rejects_in_u64_values_above_i64_max() {
    let filter = MetadataFilter::In(
        "count".to_string(),
        vec![json!(1_u64), json!(9_223_372_036_854_775_808_u64)],
    );

    let err = to_qdrant_filter(&filter).expect_err("overflowing u64 values should fail");

    assert!(matches!(
        err,
        QdrantStoreError::UnsupportedFilterValue { ref key, ref reason }
            if key == "count" && reason.contains("u64 value exceeds i64::MAX")
    ));
}

#[test]
fn serializes_nested_all_any_payload_for_search_wiring() {
    let metadata_filter = MetadataFilter::All(vec![
        MetadataFilter::Eq("source".to_string(), json!("guide")),
        MetadataFilter::Any(vec![
            MetadataFilter::In("count".to_string(), vec![json!(1_u64), json!(2_u64)]),
            MetadataFilter::Range {
                key: "score".to_string(),
                min: Some(json!(0.75)),
                max: None,
            },
        ]),
    ]);

    let qdrant_filter = to_qdrant_filter(&metadata_filter).expect("metadata filter should convert");
    let payload =
        qdrant_filter_to_payload(&qdrant_filter).expect("payload serialization should work");

    assert_eq!(
        payload,
        json!({
            "must": [
                {
                    "must": [
                        {
                            "key": "source",
                            "match": { "value": "guide" }
                        }
                    ]
                },
                {
                    "should": [
                        {
                            "must": [
                                {
                                    "key": "count",
                                    "match": { "any": [1, 2] }
                                }
                            ]
                        },
                        {
                            "must": [
                                {
                                    "key": "score",
                                    "range": { "gte": 0.75 }
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    );
}
