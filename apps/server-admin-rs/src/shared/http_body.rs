use std::string::FromUtf8Error;

use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ResponseBodyReadError {
    #[error("HTTP response body read failed: {0}")]
    Read(#[from] reqwest::Error),
    #[error("HTTP response body exceeds {limit} bytes")]
    TooLarge { limit: usize },
    #[error("HTTP response body is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] FromUtf8Error),
    #[error("HTTP response body is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

pub(crate) async fn read_response_bytes_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, ResponseBodyReadError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(ResponseBodyReadError::TooLarge { limit });
    }
    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or_default()
        .min(limit);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await? {
        append_limited(&mut body, &chunk, limit)?;
    }
    Ok(body)
}

pub(crate) async fn read_response_text_limited(
    response: reqwest::Response,
    limit: usize,
) -> Result<String, ResponseBodyReadError> {
    String::from_utf8(read_response_bytes_limited(response, limit).await?)
        .map_err(ResponseBodyReadError::from)
}

pub(crate) async fn read_response_json_limited<T>(
    response: reqwest::Response,
    limit: usize,
) -> Result<T, ResponseBodyReadError>
where
    T: DeserializeOwned,
{
    Ok(serde_json::from_slice(
        &read_response_bytes_limited(response, limit).await?,
    )?)
}

pub(crate) async fn read_response_text_prefix(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<String, ResponseBodyReadError> {
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(limit),
    );
    while body.len() < limit {
        let Some(chunk) = response.chunk().await? else {
            break;
        };
        let remaining = limit - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn append_limited(
    body: &mut Vec<u8>,
    chunk: &[u8],
    limit: usize,
) -> Result<(), ResponseBodyReadError> {
    if chunk.len() > limit.saturating_sub(body.len()) {
        return Err(ResponseBodyReadError::TooLarge { limit });
    }
    body.extend_from_slice(chunk);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ResponseBodyReadError, append_limited};

    #[test]
    fn limited_body_accepts_the_exact_limit() {
        let mut body = vec![1, 2];
        append_limited(&mut body, &[3, 4], 4).unwrap();
        assert_eq!(body, vec![1, 2, 3, 4]);
    }

    #[test]
    fn limited_body_rejects_a_chunk_before_allocating_past_the_limit() {
        let mut body = vec![1, 2];
        let result = append_limited(&mut body, &[3, 4, 5], 4);
        assert!(matches!(
            result,
            Err(ResponseBodyReadError::TooLarge { limit: 4 })
        ));
        assert_eq!(body, vec![1, 2]);
    }
}
