use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_pinecone::client::PineconeHttpClient;

#[tokio::test]
async fn upsert_sends_api_key_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/vectors/upsert"))
        .and(header("Api-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = PineconeHttpClient::new(server.uri(), "test-key".to_string()).unwrap();
    let payload = json!({"vectors": [], "namespace": "prod"});
    client
        .post_json("/vectors/upsert", &payload)
        .await
        .unwrap();
}

#[tokio::test]
async fn maps_api_error_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({"message": "rate limit"})))
        .mount(&server)
        .await;

    let client = PineconeHttpClient::new(server.uri(), "test-key".to_string()).unwrap();
    let err = client.post_json("/query", &json!({})).await.unwrap_err();
    assert!(err.to_string().contains("429"));
}
