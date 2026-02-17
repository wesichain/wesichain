use httpmock::prelude::*;
use serde_json::json;
use wesichain_core::MetadataFilter;
use wesichain_core::VectorStore;
use wesichain_weaviate::{
    filter::to_weaviate_filter, mapper::build_near_vector_query, WeaviateStoreError,
    WeaviateVectorStore,
};

#[test]
fn converts_eq_filter() {
    let filter = MetadataFilter::Eq("source".to_string(), json!("tweet"));

    let out = to_weaviate_filter(&filter).expect("eq filter should convert");

    assert_eq!(
        out,
        "{operator:Equal,path:[\"source\"],valueText:\"tweet\"}"
    );
}

#[test]
fn converts_in_filter() {
    let filter = MetadataFilter::In("lang".to_string(), vec![json!("en"), json!("de")]);

    let out = to_weaviate_filter(&filter).expect("in filter should convert");

    assert_eq!(
        out,
        "{operator:ContainsAny,path:[\"lang\"],valueTextArray:[\"en\",\"de\"]}"
    );
}

#[test]
fn converts_range_filter() {
    let filter = MetadataFilter::Range {
        key: "score".to_string(),
        min: Some(json!(0.25)),
        max: Some(json!(0.75)),
    };

    let out = to_weaviate_filter(&filter).expect("range filter should convert");

    assert_eq!(
        out,
        "{operator:And,operands:[{operator:GreaterThanEqual,path:[\"score\"],valueNumber:0.25},{operator:LessThanEqual,path:[\"score\"],valueNumber:0.75}]}"
    );
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

    let out = to_weaviate_filter(&filter).expect("nested all/any filter should convert");

    assert_eq!(
        out,
        "{operator:And,operands:[{operator:Equal,path:[\"source\"],valueText:\"guide\"},{operator:Or,operands:[{operator:ContainsAny,path:[\"lang\"],valueTextArray:[\"en\",\"de\"]},{operator:GreaterThanEqual,path:[\"score\"],valueNumber:0.9}]}]}"
    );
}

#[test]
fn translates_nested_path_segments() {
    let filter = MetadataFilter::Eq("source.env".to_string(), json!("prod"));

    let out = to_weaviate_filter(&filter).expect("nested key should convert");

    assert_eq!(
        out,
        "{operator:Equal,path:[\"source\",\"env\"],valueText:\"prod\"}"
    );
}

#[test]
fn rejects_malformed_metadata_paths_with_empty_segments() {
    for malformed_key in [".source", "source.", "source..env", "."] {
        let filter = MetadataFilter::Eq(malformed_key.to_string(), json!("prod"));

        let err = to_weaviate_filter(&filter).expect_err("malformed metadata key should fail");

        assert!(matches!(
            err,
            WeaviateStoreError::UnsupportedFilterValue { ref key, ref reason }
                if key == malformed_key && reason.contains("empty path segment")
        ));
    }
}

#[test]
fn rejects_unsupported_value_types_with_typed_error() {
    let filter = MetadataFilter::Eq("obj".to_string(), json!({"x": 1}));

    let err = to_weaviate_filter(&filter).expect_err("object eq should fail");

    assert!(matches!(
        err,
        WeaviateStoreError::UnsupportedFilterValue { ref key, ref reason }
            if key == "obj" && reason.contains("object")
    ));
}

#[tokio::test]
async fn search_includes_translated_where_clause() {
    let server = MockServer::start();
    let store = WeaviateVectorStore::builder()
        .base_url(server.base_url())
        .class_name("Doc")
        .build()
        .expect("store should build");

    let filter = MetadataFilter::Eq("source.env".to_string(), json!("prod"));

    let search = server.mock(|when, then| {
        when.method(POST).path("/v1/graphql").body_contains(
            "where:{operator:Equal,path:[\\\"source\\\",\\\"env\\\"],valueText:\\\"prod\\\"}",
        );
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "data": {
                    "Get": {
                        "Doc": []
                    }
                }
            }));
    });

    let results = store
        .search(&[1.0, 0.0, 0.0], 2, Some(&filter))
        .await
        .expect("search with filter should succeed");

    assert!(results.is_empty());
    search.assert();
}

#[test]
fn query_builder_includes_optional_where_clause() {
    let query = build_near_vector_query(
        "Doc",
        &[1.0, 0.0],
        3,
        Some("{operator:Equal,path:[\"source\",\"env\"],valueText:\"prod\"}"),
    );

    assert!(
        query.contains("where:{operator:Equal,path:[\"source\",\"env\"],valueText:\"prod\"}"),
        "query should include translated where clause"
    );
}
