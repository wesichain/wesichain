use wesichain_core::{MetadataFilter, Value};

#[test]
fn metadata_filter_roundtrip() {
    let filter = MetadataFilter::All(vec![
        MetadataFilter::Eq("tag".to_string(), Value::String("alpha".to_string())),
        MetadataFilter::Any(vec![
            MetadataFilter::Range {
                key: "score".to_string(),
                min: Some(Value::Number(1.into())),
                max: Some(Value::Number(10.into())),
            },
            MetadataFilter::In(
                "group".to_string(),
                vec![
                    Value::String("a".to_string()),
                    Value::String("b".to_string()),
                ],
            ),
        ]),
    ]);

    let json = serde_json::to_string(&filter).unwrap();
    let parsed: MetadataFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(filter, parsed);
}
