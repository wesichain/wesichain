use serde_json::json;
use wesichain_core::MetadataFilter;
use wesichain_pinecone::filter::{to_pinecone_filter_json, PineconeFilter};

#[test]
fn converts_eq_filter() {
    let filter = PineconeFilter::Typed(MetadataFilter::Eq("source".to_string(), json!("tweet")));
    let out = to_pinecone_filter_json(&filter).unwrap();
    assert_eq!(out, json!({"source": {"$eq": "tweet"}}));
}

#[test]
fn converts_nested_all_any_filter() {
    let filter = PineconeFilter::Typed(MetadataFilter::All(vec![
        MetadataFilter::Eq("source".to_string(), json!("tweet")),
        MetadataFilter::Any(vec![
            MetadataFilter::In("lang".to_string(), vec![json!("en"), json!("id")]),
            MetadataFilter::Range {
                key: "score".to_string(),
                min: Some(json!(0.5)),
                max: None,
            },
        ]),
    ]));
    let out = to_pinecone_filter_json(&filter).unwrap();
    assert!(out.get("$and").is_some());
}

#[test]
fn raw_filter_passthrough() {
    let raw = json!({"$or": [{"source": {"$eq": "tweet"}}]});
    let out = to_pinecone_filter_json(&PineconeFilter::Raw(raw.clone())).unwrap();
    assert_eq!(out, raw);
}
