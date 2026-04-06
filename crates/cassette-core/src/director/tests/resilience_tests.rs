use crate::director::download::resume::is_retryable;
use crate::director::error::DirectorError;
use crate::director::resilience::range_request::with_optional_range;

#[test]
fn retryable_error_classification() {
    assert!(is_retryable(&DirectorError::NetworkError("x".to_string())));
    assert!(is_retryable(&DirectorError::HttpError(503)));
    assert!(!is_retryable(&DirectorError::FileTooLarge {
        size: 100,
        max: 10
    }));
}

#[test]
fn range_header_is_added_for_partial_size() {
    let client = reqwest::Client::new();
    let request = client.get("https://example.com/file");
    let request = with_optional_range(request, 128).build().expect("request");
    assert!(request.headers().contains_key(reqwest::header::RANGE));
}
