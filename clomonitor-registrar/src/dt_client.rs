use anyhow::Result;
use async_trait::async_trait;
use reqwest::StatusCode;
use tokio::time::{sleep, Duration};
use tracing::debug;

use crate::dt_types::{DtComponent, DtProject};

const PAGE_SIZE: u32 = 100;
const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 1000;

#[async_trait]
pub(crate) trait DtClient {
    async fn get_projects(&self) -> Result<Vec<DtProject>>;
    async fn get_project_components(&self, project_uuid: &str) -> Result<Vec<DtComponent>>;
}

pub(crate) struct DtHttpClient {
    base_url: String,
    api_key: String,
    http_client: reqwest::Client,
}

impl DtHttpClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http_client: reqwest::Client::new(),
        }
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    async fn fetch_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<(T, usize)> {
        let mut retry_count = 0;

        loop {
            let response = self
                .http_client
                .get(url)
                .header("X-Api-Key", &self.api_key)
                .send()
                .await?;

            match response.status() {
                StatusCode::OK => {
                    let total_count = response
                        .headers()
                        .get("X-Total-Count")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);

                    let data: T = response.json().await?;
                    return Ok((data, total_count));
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    if retry_count >= MAX_RETRIES {
                        return Err(anyhow::format_err!(
                            "DT API rate limit exceeded after {} retries",
                            MAX_RETRIES
                        ));
                    }

                    let retry_after = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(INITIAL_RETRY_DELAY_MS / 1000);

                    let delay_ms = if retry_after > 0 {
                        retry_after * 1000
                    } else {
                        INITIAL_RETRY_DELAY_MS * 2_u64.pow(retry_count)
                    };

                    debug!(
                        "Rate limited (429), retrying in {}ms (attempt {}/{})",
                        delay_ms,
                        retry_count + 1,
                        MAX_RETRIES
                    );

                    sleep(Duration::from_millis(delay_ms)).await;
                    retry_count += 1;
                }
                _ => {
                    return Err(anyhow::format_err!(
                        "DT API error: status={}, body={}",
                        response.status(),
                        response.text().await?
                    ));
                }
            }
        }
    }
}

#[async_trait]
impl DtClient for DtHttpClient {
    async fn get_projects(&self) -> Result<Vec<DtProject>> {
        let mut all_projects = Vec::new();
        let mut page = 1;

        loop {
            let url = self.build_url(&format!(
                "/v1/project?pageNumber={}&pageSize={}",
                page, PAGE_SIZE
            ));

            debug!("Fetching projects page {} from {}", page, url);

            let (projects, total_count): (Vec<DtProject>, usize) =
                self.fetch_with_retry(&url).await?;

            let fetched = projects.len();
            all_projects.extend(projects);

            debug!(
                "Fetched {} projects (total so far: {}/{})",
                fetched,
                all_projects.len(),
                total_count
            );

            // Stop if we've fetched everything
            if all_projects.len() >= total_count {
                break;
            }

            // Stop if we got no results on this page
            if fetched == 0 {
                break;
            }

            page += 1;
        }

        Ok(all_projects)
    }

    async fn get_project_components(&self, project_uuid: &str) -> Result<Vec<DtComponent>> {
        let mut all_components = Vec::new();
        let mut page = 1;

        loop {
            let url = self.build_url(&format!(
                "/v1/component/project/{}?pageNumber={}&pageSize={}",
                project_uuid, page, PAGE_SIZE
            ));

            debug!(
                "Fetching components page {} for project {}",
                page, project_uuid
            );

            let (components, total_count): (Vec<DtComponent>, usize) =
                self.fetch_with_retry(&url).await?;

            let fetched = components.len();
            all_components.extend(components);

            debug!(
                "Fetched {} components (total so far: {}/{})",
                fetched,
                all_components.len(),
                total_count
            );

            // Stop if we've fetched everything
            if all_components.len() >= total_count {
                break;
            }

            // Stop if we got no results on this page
            if fetched == 0 {
                break;
            }

            page += 1;
        }

        Ok(all_components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    #[tokio::test]
    async fn test_get_projects_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/v1/project")
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(r#"[{"uuid":"123","name":"test-project","version":"1.0"}]"#)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let projects = client.get_projects().await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_pagination() {
        let mut server = mockito::Server::new_async().await;

        // First page
        let mock1 = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()))
            .with_status(200)
            .with_header("X-Total-Count", "2")
            .with_body(r#"[{"uuid":"1","name":"proj1"}]"#)
            .create_async()
            .await;

        // Second page
        let mock2 = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::UrlEncoded("pageNumber".into(), "2".into()))
            .with_status(200)
            .with_header("X-Total-Count", "2")
            .with_body(r#"[{"uuid":"2","name":"proj2"}]"#)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let projects = client.get_projects().await.unwrap();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "proj1");
        assert_eq!(projects[1].name, "proj2");
        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_projects_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/v1/project")
            .with_status(403)
            .with_body("Forbidden")
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "bad-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Check that error contains status code or error info
        assert!(err_msg.contains("API error") || err_msg.contains("status="), "Error message was: {}", err_msg);
    }

    #[tokio::test]
    async fn test_get_project_components() {
        let mut server = mockito::Server::new_async().await;
        let project_uuid = "proj-123";

        let mock = server
            .mock("GET", format!("/api/v1/component/project/{}", project_uuid).as_str())
            .match_header("X-Api-Key", "test-key")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body(
                r#"[{"uuid":"comp-1","name":"lodash","version":"4.17.21","classifier":"LIBRARY"}]"#,
            )
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let components = client.get_project_components(project_uuid).await.unwrap();

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name, "lodash");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dt_client_handles_429_rate_limit() {
        let mut server = mockito::Server::new_async().await;

        let mock_fail = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(429)
            .with_header("Retry-After", "0")
            .expect(1)
            .create_async()
            .await;

        let mock_success = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "0")
            .with_body("[]")
            .expect(1)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        mock_fail.assert_async().await;
        mock_success.assert_async().await;
    }

    #[tokio::test]
    async fn test_dt_client_retry_exhaustion() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(429)
            .with_header("Retry-After", "0")
            .expect(4) // Initial attempt + 3 retries
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("rate limit") || err_msg.contains("429"),
            "Error message was: {}",
            err_msg
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dt_client_handles_invalid_json() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            .with_header("X-Total-Count", "1")
            .with_body("not json at all")
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("JSON")
                || err_msg.contains("parse")
                || err_msg.contains("expected")
                || err_msg.contains("decoding"),
            "Error message was: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_dt_client_handles_missing_total_count() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/v1/project")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("pageNumber".into(), "1".into()),
                mockito::Matcher::UrlEncoded("pageSize".into(), "100".into()),
            ]))
            .with_status(200)
            // No X-Total-Count header
            .with_body(r#"[{"uuid":"123","name":"test-project","version":"1.0"}]"#)
            .create_async()
            .await;

        let client = DtHttpClient::new(server.url(), "test-key".to_string());
        let result = client.get_projects().await;

        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");
    }
}
