use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use wesichain_langsmith::LangSmithClient;

#[tokio::test]
async fn create_run_includes_idempotency_key() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let payload = json!({"id": run_id, "name": "demo"});

    Mock::given(method("POST"))
        .and(path("/runs"))
        .and(header("x-idempotency-key", run_id.to_string()))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = LangSmithClient::new(server.uri(), SecretString::new("test-key".to_string()));
    client.create_run(run_id, &payload).await.unwrap();
}

#[tokio::test]
async fn patch_run_is_partial_payload() {
    let server = MockServer::start().await;
    let run_id = Uuid::new_v4();
    let payload = json!({
        "end_time": "2026-02-03T00:00:00Z",
        "outputs": {"value": 4}
    });

    Mock::given(method("PATCH"))
        .and(path(format!("/runs/{}", run_id)))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = LangSmithClient::new(server.uri(), SecretString::new("test-key".to_string()));
    client.update_run(run_id, &payload).await.unwrap();
}
