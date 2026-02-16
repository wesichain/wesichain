use serde_json::{Map as JsonMap, Value as JsonValue};
use wesichain_qdrant::mapper::{scored_point_to_result, PointId, ScoredPoint, CONTENT_PAYLOAD_KEY};
use wesichain_qdrant::QdrantStoreError;

#[test]
fn scored_point_to_result_errors_when_content_payload_is_missing() {
    let point = ScoredPoint {
        id: PointId::String("doc-1".to_string()),
        score: 0.9,
        payload: JsonMap::new(),
    };

    let err = scored_point_to_result(point).expect_err("missing content payload should error");

    assert!(matches!(
        err,
        QdrantStoreError::MissingContentPayload { point_id } if point_id == "doc-1"
    ));
}

#[test]
fn scored_point_to_result_errors_when_content_payload_is_not_string() {
    let mut payload = JsonMap::new();
    payload.insert(CONTENT_PAYLOAD_KEY.to_string(), JsonValue::Bool(true));

    let point = ScoredPoint {
        id: PointId::String("doc-2".to_string()),
        score: 0.8,
        payload,
    };

    let err = scored_point_to_result(point).expect_err("non-string content payload should error");

    assert!(matches!(
        err,
        QdrantStoreError::InvalidContentPayloadType {
            point_id,
            expected,
            actual,
        } if point_id == "doc-2" && expected == "string" && actual == "boolean"
    ));
}
