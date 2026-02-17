use std::error::Error as StdError;

use wesichain_core::StoreError;
use wesichain_weaviate::WeaviateStoreError;

#[test]
fn invalid_document_id_maps_to_invalid_id_store_error() {
    let err = StoreError::from(WeaviateStoreError::InvalidDocumentId("bad-id".to_string()));

    assert!(matches!(err, StoreError::InvalidId(id) if id == "bad-id"));
}

#[test]
fn request_errors_map_to_internal_and_preserve_source() {
    let request_error = reqwest::Client::new()
        .get(":://not-a-valid-url")
        .build()
        .expect_err("invalid URL should produce reqwest error");
    let original_message = request_error.to_string();

    let err = StoreError::from(WeaviateStoreError::Request(request_error));

    match err {
        StoreError::Internal(inner) => {
            let mapped = inner
                .downcast_ref::<WeaviateStoreError>()
                .expect("boxed error should preserve WeaviateStoreError type");

            match mapped {
                WeaviateStoreError::Request(source) => {
                    assert_eq!(source.to_string(), original_message);
                }
                other => panic!("expected request variant, got: {other:?}"),
            }

            assert!(
                StdError::source(mapped).is_some(),
                "request error should be preserved as source"
            );
        }
        other => panic!("expected internal error, got: {other:?}"),
    }
}

#[test]
fn backend_errors_map_to_internal_with_original_variant_preserved() {
    let err = StoreError::from(WeaviateStoreError::HttpStatus {
        status: 503,
        message: "upstream unavailable".to_string(),
    });

    match err {
        StoreError::Internal(inner) => {
            let mapped = inner
                .downcast_ref::<WeaviateStoreError>()
                .expect("boxed error should preserve WeaviateStoreError type");

            assert!(matches!(
                mapped,
                WeaviateStoreError::HttpStatus { status, message }
                    if *status == 503 && message == "upstream unavailable"
            ));
        }
        other => panic!("expected internal error, got: {other:?}"),
    }
}
