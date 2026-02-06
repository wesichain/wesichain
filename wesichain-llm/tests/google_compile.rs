#[cfg(feature = "google")]
#[test]
fn google_client_compiles() {
    use wesichain_llm::GoogleClient;

    let _ = GoogleClient::new("test-key", "gemini-1.5-flash");
}
