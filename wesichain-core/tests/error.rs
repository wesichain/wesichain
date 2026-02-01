use wesichain_core::WesichainError;

#[test]
fn error_display_for_max_retries() {
    let err = WesichainError::MaxRetriesExceeded { max: 2 };
    assert_eq!(format!("{err}"), "Max retries (2) exceeded");
}
