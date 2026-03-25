use reqwest::header::RANGE;

pub fn with_optional_range(
    mut request: reqwest::RequestBuilder,
    existing_size: u64,
) -> reqwest::RequestBuilder {
    if existing_size > 0 {
        request = request.header(RANGE, format!("bytes={existing_size}-"));
    }
    request
}
