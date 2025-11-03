use anyhow::Result;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use tracing::{debug, warn};

use super::{normalize_git_url, parse_maven_purl, RegistryApi};

const MAVEN_CENTRAL_URL: &str = "https://repo1.maven.org/maven2";

pub struct MavenRegistry {
    client: Client,
    base_url: String,
}

impl MavenRegistry {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: MAVEN_CENTRAL_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub fn new_with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    /// Build Maven Central POM URL from group, artifact, and version
    /// Example: org.springframework/spring-core/5.3.0 ->
    ///   https://repo1.maven.org/maven2/org/springframework/spring-core/5.3.0/spring-core-5.3.0.pom
    fn build_pom_url(&self, group_id: &str, artifact_id: &str, version: &str) -> String {
        let group_path = group_id.replace('.', "/");
        format!(
            "{}/{}/{}/{}/{}-{}.pom",
            self.base_url, group_path, artifact_id, version, artifact_id, version
        )
    }

    async fn fetch_pom(&self, group_id: &str, artifact_id: &str, version: &str) -> Result<Option<String>> {
        let url = self.build_pom_url(group_id, artifact_id, version);
        debug!("Fetching Maven POM from: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            debug!("POM not found: {}:{}:{}", group_id, artifact_id, version);
            return Ok(None);
        }

        let pom_content = response.text().await?;
        Ok(Some(pom_content))
    }

    fn extract_repository_url_from_pom(&self, pom_xml: &str) -> Option<String> {
        let mut reader = Reader::from_str(pom_xml);
        reader.trim_text(true);

        let mut buf = Vec::new();

        // Track multiple URL sources
        let mut scm_url = String::new();
        let mut scm_connection = String::new();
        let mut project_url = String::new();
        let mut issue_url = String::new();

        // State tracking
        let mut in_scm = false;
        let mut in_project = false;
        let mut in_issue_mgmt = false;
        let mut in_url = false;
        let mut in_connection = false;
        let mut depth = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth += 1;

                    match name.as_str() {
                        "project" => in_project = true,
                        "scm" => in_scm = true,
                        "issueManagement" => in_issue_mgmt = true,
                        "url" => in_url = true,
                        "connection" => {
                            if in_scm {
                                in_connection = true;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth -= 1;

                    match name.as_str() {
                        "scm" => in_scm = false,
                        "issueManagement" => in_issue_mgmt = false,
                        "url" => in_url = false,
                        "connection" => in_connection = false,
                        _ => {}
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_url {
                        let text = e.unescape().ok()?.to_string();

                        // Categorize by context
                        if in_scm && scm_url.is_empty() {
                            scm_url = text;
                        } else if in_project && depth == 2 && project_url.is_empty() {
                            // depth == 2 means direct child of <project>
                            project_url = text;
                        } else if in_issue_mgmt && issue_url.is_empty() {
                            issue_url = text;
                        }
                    } else if in_connection && in_scm && scm_connection.is_empty() {
                        scm_connection = e.unescape().ok()?.to_string();
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn!("Error parsing POM XML: {}", e);
                    return None;
                }
                _ => {}
            }
            buf.clear();
        }

        // Priority fallback chain with validation

        // Priority 1: <scm><url>
        if !scm_url.is_empty()
            && let Some(normalized) = self.validate_and_normalize(&scm_url)
        {
            return Some(normalized);
        }

        // Priority 2: <scm><connection>
        if !scm_connection.is_empty() {
            let clean = scm_connection
                .strip_prefix("scm:git:")
                .unwrap_or(&scm_connection);
            if let Some(normalized) = self.validate_and_normalize(clean) {
                return Some(normalized);
            }
        }

        // Priority 3: <project><url> (NEW!)
        if !project_url.is_empty()
            && let Some(normalized) = self.validate_and_normalize(&project_url)
        {
            debug!("Found repository in project URL: {}", normalized);
            return Some(normalized);
        }

        // Priority 4: <issueManagement><url> as last resort
        // Only if it's a GitHub Issues URL
        if !issue_url.is_empty() && issue_url.contains("github.com") && issue_url.contains("/issues") {
            let repo_url = issue_url.replace("/issues", "");
            if let Some(normalized) = self.validate_and_normalize(&repo_url) {
                debug!("Found repository from issue tracker: {}", normalized);
                return Some(normalized);
            }
        }

        None
    }

    fn validate_and_normalize(&self, url: &str) -> Option<String> {
        // Normalize the URL
        let normalized = normalize_git_url(url)?;

        // Reject non-repository URLs
        if Self::is_documentation_site(&normalized) {
            debug!("Rejected documentation site: {}", normalized);
            return None;
        }

        if Self::is_issue_tracker_only(&normalized) {
            debug!("Rejected issue tracker: {}", normalized);
            return None;
        }

        Some(normalized)
    }

    fn is_documentation_site(url: &str) -> bool {
        url.contains("readthedocs.io")
            || url.contains(".github.io/")
            || url.contains("javadoc.io")
            || url.contains("docs.oracle.com")
    }

    fn is_issue_tracker_only(url: &str) -> bool {
        url.contains("jira.")
            || url.contains("bugzilla.")
            || url.contains("issues.apache.org")
            || (url.contains("/issues") && !url.contains("github.com"))
    }
}

#[async_trait]
impl RegistryApi for MavenRegistry {
    async fn lookup_repository(&self, purl: &str) -> Result<Option<String>> {
        let (group_id, artifact_id, version) = parse_maven_purl(purl)?;
        debug!("Looking up Maven artifact: {}:{}:{}", group_id, artifact_id, version);

        let Some(pom_xml) = self.fetch_pom(&group_id, &artifact_id, &version).await? else {
            return Ok(None);
        };

        if let Some(url) = self.extract_repository_url_from_pom(&pom_xml) {
            debug!("Found repository URL: {}", url);
            return Ok(Some(url));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    #[tokio::test]
    async fn test_maven_lookup_pom_with_scm() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/org/springframework/spring-core/5.3.0/spring-core-5.3.0.pom")
            .with_status(200)
            .with_header("content-type", "application/xml")
            .with_body(include_str!("testdata/maven_spring_core.pom"))
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:maven/org.springframework/spring-core@5.3.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://github.com/spring-projects/spring-framework".to_string())
        );
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_maven_lookup_pom_with_gitlab() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/org/gitlab/gitlab-library/2.0.0/gitlab-library-2.0.0.pom")
            .with_status(200)
            .with_header("content-type", "application/xml")
            .with_body(include_str!("testdata/maven_gitlab.pom"))
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:maven/org.gitlab/gitlab-library@2.0.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://gitlab.com/gitlab-org/gitlab-library".to_string())
        );
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_maven_lookup_pom_no_scm() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/com/example/no-scm-artifact/1.0.0/no-scm-artifact-1.0.0.pom")
            .with_status(200)
            .with_header("content-type", "application/xml")
            .with_body(include_str!("testdata/maven_no_scm.pom"))
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:maven/com.example/no-scm-artifact@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_maven_lookup_404() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/com/nonexistent/artifact/1.0.0/artifact-1.0.0.pom")
            .with_status(404)
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:maven/com.nonexistent/artifact@1.0.0")
            .await
            .unwrap();

        assert_eq!(result, None);
        mock_server.assert_async().await;
    }

    #[tokio::test]
    async fn test_maven_purl_with_qualifiers() {
        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/org/springframework/spring-core/5.3.0/spring-core-5.3.0.pom")
            .with_status(200)
            .with_header("content-type", "application/xml")
            .with_body(include_str!("testdata/maven_spring_core.pom"))
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        // PURL with qualifiers - should strip them
        let result = client
            .lookup_repository("pkg:maven/org.springframework/spring-core@5.3.0?type=jar")
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(
            result,
            Some("https://github.com/spring-projects/spring-framework".to_string())
        );
        mock_server.assert_async().await;
    }

    #[test]
    fn test_build_pom_url() {
        let client = MavenRegistry::new();
        let url = client.build_pom_url("org.springframework", "spring-core", "5.3.0");
        assert_eq!(
            url,
            "https://repo1.maven.org/maven2/org/springframework/spring-core/5.3.0/spring-core-5.3.0.pom"
        );

        // Test with single-segment group
        let url2 = client.build_pom_url("junit", "junit", "4.13.2");
        assert_eq!(
            url2,
            "https://repo1.maven.org/maven2/junit/junit/4.13.2/junit-4.13.2.pom"
        );
    }

    #[tokio::test]
    async fn test_maven_project_url_fallback() {
        // Test docker-java-api case
        let pom = r#"<?xml version="1.0"?>
<project>
    <name>docker-java-api</name>
    <url>https://github.com/docker-java/docker-java</url>
    <description>Java API Client for Docker</description>
    <!-- No SCM section -->
</project>"#;

        let mut _mock = mockito::Server::new_async().await;
        let mock_server = _mock
            .mock("GET", "/com/github/docker-java/docker-java-api/6.2.0/docker-java-api-6.2.0.pom")
            .with_status(200)
            .with_body(pom)
            .create_async()
            .await;

        let client = MavenRegistry::new_with_base_url(_mock.url());
        let result = client
            .lookup_repository("pkg:maven/com.github.docker-java/docker-java-api@6.2.0")
            .await
            .unwrap();

        assert_eq!(
            result,
            Some("https://github.com/docker-java/docker-java".to_string())
        );
        mock_server.assert_async().await;
    }

    #[test]
    fn test_maven_scm_takes_priority_over_project_url() {
        // SCM should win over project URL
        let pom = r#"<?xml version="1.0"?>
<project>
    <url>https://example.com/docs</url>
    <scm>
        <url>https://github.com/real/repo</url>
    </scm>
</project>"#;

        let client = MavenRegistry::new();
        let url = client.extract_repository_url_from_pom(pom);

        assert_eq!(url, Some("https://github.com/real/repo".to_string()));
    }

    #[test]
    fn test_maven_rejects_documentation_urls() {
        let pom = r#"<?xml version="1.0"?>
<project>
    <url>https://project.github.io/docs</url>
</project>"#;

        let client = MavenRegistry::new();
        let url = client.extract_repository_url_from_pom(pom);

        assert_eq!(url, None); // Should reject github.io pages
    }

    #[test]
    fn test_maven_extracts_from_issue_tracker() {
        let pom = r#"<?xml version="1.0"?>
<project>
    <issueManagement>
        <url>https://github.com/some/project/issues</url>
    </issueManagement>
</project>"#;

        let client = MavenRegistry::new();
        let url = client.extract_repository_url_from_pom(pom);

        assert_eq!(url, Some("https://github.com/some/project".to_string()));
    }

    #[test]
    fn test_maven_handles_nested_urls() {
        // Make sure we only capture project-level URL, not nested ones
        let pom = r#"<?xml version="1.0"?>
<project>
    <url>https://github.com/correct/repo</url>
    <dependencies>
        <dependency>
            <url>https://github.com/wrong/dep</url>
        </dependency>
    </dependencies>
</project>"#;

        let client = MavenRegistry::new();
        let url = client.extract_repository_url_from_pom(pom);

        assert_eq!(url, Some("https://github.com/correct/repo".to_string()));
    }
}
