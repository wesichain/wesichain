use wesichain_core::ToolError;

#[test]
fn tool_error_is_displayable() {
    let err = ToolError::InvalidInput("missing field".to_string());
    assert!(err.to_string().contains("missing field"));
}
