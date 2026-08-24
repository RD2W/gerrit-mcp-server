// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Gerrit REST API HTTP client.
//!
//! Implements [`GerritRepository`] using `reqwest` with configurable
//! authentication (HTTP Basic, Bearer token, or GitCookies) and TLS settings.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::domain::*;

use super::auth::{AuthMode, apply_auth, normalize_url_for_auth};
use super::tls::{self, TlsConfig};

const XSSI_PREFIX: &str = ")]}'\n";

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

/// Configuration for [`GerritClient`].
#[derive(Debug, Clone)]
pub struct GerritClientConfig {
    pub base_url: String,
    pub auth: AuthMode,
    pub timeout: Duration,
    pub tls: TlsConfig,
    /// When `true`, skips URL normalization (http→https upgrade, `/a` prefix).
    /// Intended for testing with local mock servers.
    #[doc(hidden)]
    pub disable_url_normalization: bool,
}

// ---------------------------------------------------------------------------
// Gerrit HTTP client
// ---------------------------------------------------------------------------

/// HTTP client for the Gerrit REST API.
#[derive(Debug, Clone)]
pub struct GerritClient {
    client: Client,
    base_url: String,
    auth: AuthMode,
}

impl GerritClient {
    /// Creates a new client with the given configuration.
    ///
    /// # Errors
    /// Returns [`DomainError::Tls`] if the TLS connector cannot be built.
    pub fn new(config: GerritClientConfig) -> Result<Self, DomainError> {
        let tls_config = tls::build_tls_connector(&config.tls)?;

        let base_url = if config.disable_url_normalization {
            config.base_url.trim_end_matches('/').to_string()
        } else {
            normalize_url_for_auth(config.base_url, &config.auth)
        };

        let client = Client::builder()
            .timeout(config.timeout)
            .use_preconfigured_tls(tls_config)
            .build()
            .map_err(|e| DomainError::Tls(format!("failed to build HTTP client: {e}")))?;

        tracing::info!(
            base_url = %base_url,
            auth_mode = ?config.auth,
            verify_ssl = config.tls.verify_ssl,
            "Gerrit client initialized"
        );

        Ok(Self {
            client,
            base_url,
            auth: config.auth,
        })
    }

    // -- URL construction ---------------------------------------------------

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    // -- Builder helpers ----------------------------------------------------

    fn get_builder(&self, url: &str) -> RequestBuilder {
        let builder = self.client.get(url).header("Accept", "application/json");
        apply_auth(builder, &self.base_url, &self.auth)
    }

    fn post_builder(&self, url: &str, body: &impl Serialize) -> RequestBuilder {
        let builder = self
            .client
            .post(url)
            .json(body)
            .header("Accept", "application/json");
        apply_auth(builder, &self.base_url, &self.auth)
    }

    fn put_builder(&self, url: &str, body: &impl Serialize) -> RequestBuilder {
        let builder = self
            .client
            .put(url)
            .json(body)
            .header("Accept", "application/json");
        apply_auth(builder, &self.base_url, &self.auth)
    }

    fn delete_builder(&self, url: &str) -> RequestBuilder {
        apply_auth(
            self.client.delete(url).header("Accept", "application/json"),
            &self.base_url,
            &self.auth,
        )
    }

    // -- Response helpers ---------------------------------------------------

    /// Strip the Gerrit XSSI prefix `)]}'\n` from a response body.
    fn strip_xssi(body: &str) -> &str {
        body.strip_prefix(XSSI_PREFIX).unwrap_or(body)
    }

    fn json_decode_error(e: impl std::fmt::Display, body: &str) -> DomainError {
        let truncated: String = body.chars().take(500).collect();
        DomainError::Decode(format!(
            "JSON parse error: {e}\nRaw body (500 chars): {truncated}"
        ))
    }

    /// Send a request and check for non-2xx status, consuming the body on error.
    async fn check_response(response: reqwest::Response) -> Result<reqwest::Response, DomainError> {
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(DomainError::HttpStatus { status, body });
        }
        Ok(response)
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, DomainError> {
        let response = self.get_builder(url).send().await?;
        let response = Self::check_response(response).await?;
        let body = response.text().await?;
        let trimmed = Self::strip_xssi(&body);

        serde_json::from_str(trimmed).map_err(|e| Self::json_decode_error(e, &body))
    }

    async fn get_raw(&self, url: &str) -> Result<String, DomainError> {
        let response = self.get_builder(url).send().await?;
        let response = Self::check_response(response).await?;
        let body = response.text().await?;
        Ok(Self::strip_xssi(&body).to_string())
    }

    async fn post_json<T: DeserializeOwned>(
        &self,
        url: &str,
        body: &impl Serialize,
    ) -> Result<T, DomainError> {
        let response = self.post_builder(url, body).send().await?;
        let response = Self::check_response(response).await?;
        let text = response.text().await?;
        let trimmed = Self::strip_xssi(&text);

        serde_json::from_str(trimmed).map_err(|e| Self::json_decode_error(e, &text))
    }

    async fn post_empty(&self, url: &str, body: &impl Serialize) -> Result<(), DomainError> {
        let response = self.post_builder(url, body).send().await?;
        Self::check_response(response).await?;
        Ok(())
    }

    #[allow(dead_code)]
    async fn put_empty(&self, url: &str, body: &impl Serialize) -> Result<(), DomainError> {
        let response = self.put_builder(url, body).send().await?;
        Self::check_response(response).await?;
        Ok(())
    }

    async fn delete_empty(&self, url: &str) -> Result<(), DomainError> {
        let response = self.delete_builder(url).send().await?;
        Self::check_response(response).await?;
        Ok(())
    }

    /// URL-encode a string for query parameters, path segments, etc.
    fn percent_encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }

    // -- Query helpers ------------------------------------------------------

    /// Build `?o=OPT1&o=OPT2` query string suffix for Gerrit option lists.
    fn build_options_query(options: &[String]) -> String {
        if options.is_empty() {
            return String::new();
        }
        let mut parts = String::from("?o=");
        let mut first = true;
        for opt in options {
            if !first {
                parts.push_str("&o=");
            }
            first = false;
            parts.push_str(&Self::percent_encode(opt));
        }
        parts
    }

    fn decode_diff_json(raw: &str) -> Result<String, DomainError> {
        use base64::Engine as _;

        #[derive(Debug, Deserialize)]
        struct DiffContentEntry {
            #[serde(default)]
            ab: Option<Vec<String>>,
            #[serde(default)]
            a: Option<Vec<String>>,
            #[serde(default)]
            b: Option<Vec<String>>,
        }

        #[derive(Debug, Deserialize)]
        struct DiffInfo {
            #[serde(default)]
            content: Vec<DiffContentEntry>,
        }

        let diff: DiffInfo = serde_json::from_str(raw)
            .map_err(|e| DomainError::Decode(format!("DiffInfo JSON parse error: {e}")))?;

        let engine = base64::engine::general_purpose::STANDARD;
        let mut lines = Vec::new();

        for entry in &diff.content {
            let field = entry.ab.as_ref().or(entry.a.as_ref()).or(entry.b.as_ref());
            if let Some(items) = field {
                for item in items {
                    let decoded = engine
                        .decode(item.as_bytes())
                        .map_err(|e| DomainError::Decode(format!("base64 decode error: {e}")))?;
                    let text = String::from_utf8(decoded)
                        .map_err(|e| DomainError::Decode(format!("utf-8 decode error: {e}")))?;
                    lines.push(text);
                }
            }
        }

        Ok(lines.concat())
    }
}

// ---------------------------------------------------------------------------
// GerritRepository implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl GerritRepository for GerritClient {
    async fn query_changes(
        &self,
        query: &str,
        limit: Option<u32>,
        options: &[String],
    ) -> Result<Vec<Change>, DomainError> {
        let q = Self::percent_encode(query);
        let limit_param = limit.map(|n| format!("&n={n}")).unwrap_or_default();
        let o = Self::build_options_query(options);
        let url = self.url(&format!("/changes/?q={q}{limit_param}{o}"));
        self.get_json(&url).await
    }

    async fn get_change_detail(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<ChangeDetail, DomainError> {
        let cid = Self::percent_encode(change_id);
        let o = Self::build_options_query(options);
        let url = self.url(&format!("/changes/{cid}/detail{o}"));
        self.get_json(&url).await
    }

    async fn get_commit_message(&self, change_id: &str) -> Result<CommitMessage, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/message"));
        self.get_json(&url).await
    }

    async fn list_files(&self, change_id: &str) -> Result<BTreeMap<String, FileInfo>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/files/"));
        self.get_json(&url).await
    }

    async fn get_diff(&self, change_id: &str, file_path: &str) -> Result<String, DomainError> {
        use base64::Engine as _;

        let cid = Self::percent_encode(change_id);
        let fp = Self::percent_encode(file_path);
        let url = self.url(&format!("/changes/{cid}/revisions/current/patch?path={fp}"));
        let raw = self.get_raw(&url).await?;

        if let Ok(text) = Self::decode_diff_json(&raw) {
            return Ok(text);
        }

        // Some Gerrit versions return the diff as a plain JSON string
        if let Ok(text) = serde_json::from_str::<String>(&raw) {
            return Ok(text);
        }

        let engine = base64::engine::general_purpose::STANDARD;
        let decoded = engine
            .decode(raw.trim().as_bytes())
            .map_err(|e| DomainError::Decode(format!("base64 decode error: {e}")))?;
        String::from_utf8(decoded)
            .map_err(|e| DomainError::Decode(format!("utf-8 decode error: {e}")))
    }

    async fn list_comments(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<Comment>>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/comments"));
        self.get_json(&url).await
    }

    async fn list_drafts(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<DraftComment>>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/drafts"));
        self.get_json(&url).await
    }

    async fn get_commit(&self, change_id: &str) -> Result<CommitInfo, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/commit"));
        self.get_json(&url).await
    }

    async fn suggest_reviewers(
        &self,
        change_id: &str,
        query: &str,
        limit: Option<u32>,
        exclude_groups: bool,
        reviewer_state: Option<&str>,
    ) -> Result<Vec<SuggestedReviewer>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let q = Self::percent_encode(query);
        let limit_param = limit.map(|n| format!("&n={n}")).unwrap_or_default();
        let eg = format!("&exclude-groups={exclude_groups}");
        let rs = reviewer_state
            .map(|s| format!("&reviewer-state={}", Self::percent_encode(s)))
            .unwrap_or_default();
        let url = self.url(&format!(
            "/changes/{cid}/suggest_reviewers?q={q}{limit_param}{eg}{rs}"
        ));
        self.get_json(&url).await
    }

    async fn changes_submitted_together(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<SubmittedTogether, DomainError> {
        let cid = Self::percent_encode(change_id);
        let o = Self::build_options_query(options);
        let url = self.url(&format!("/changes/{cid}/submitted_together{o}"));
        let response: SubmittedTogetherResponse = self.get_json(&url).await?;
        Ok(response.into())
    }

    async fn create_change(&self, payload: &CreateChangeRequest) -> Result<Change, DomainError> {
        let url = self.url("/changes/");
        self.post_json(&url, payload).await
    }

    async fn add_reviewer(
        &self,
        change_id: &str,
        payload: &AddReviewerRequest,
    ) -> Result<AddReviewerResult, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/reviewers"));
        self.post_json(&url, payload).await
    }

    async fn set_ready(&self, change_id: &str) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/ready"));
        self.post_empty(&url, &serde_json::json!({})).await
    }

    async fn set_wip(&self, change_id: &str, payload: &WipRequest) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/wip"));
        self.post_empty(&url, payload).await
    }

    async fn set_topic(
        &self,
        change_id: &str,
        payload: &TopicRequest,
    ) -> Result<Option<String>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/topic"));
        let text = self.put_text(&url, payload).await?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            let topic: String =
                serde_json::from_str(trimmed).map_err(|e| Self::json_decode_error(e, &text))?;
            Ok(Some(topic))
        }
    }

    async fn abandon_change(
        &self,
        change_id: &str,
        payload: &AbandonRequest,
    ) -> Result<Change, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/abandon"));
        self.post_json(&url, payload).await
    }

    async fn revert_change(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Change, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revert"));

        #[derive(Serialize)]
        struct RevertPayload {
            #[serde(skip_serializing_if = "Option::is_none")]
            message: Option<String>,
        }

        let body = RevertPayload {
            message: message.map(String::from),
        };
        self.post_json(&url, &body).await
    }

    async fn revert_submission(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Vec<Change>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revert_submission"));

        #[derive(Serialize)]
        struct RevertSubmissionPayload {
            #[serde(skip_serializing_if = "Option::is_none")]
            message: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RevertSubmissionResponse {
            revert_changes: Vec<Change>,
        }

        let body = RevertSubmissionPayload {
            message: message.map(String::from),
        };
        let resp: RevertSubmissionResponse = self.post_json(&url, &body).await?;
        Ok(resp.revert_changes)
    }

    async fn set_labels(&self, change_id: &str, payload: &ReviewInput) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/review"));
        self.post_empty(&url, payload).await
    }

    async fn post_review(
        &self,
        change_id: &str,
        payload: &CommentBatchInput,
    ) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/review"));
        self.post_empty(&url, payload).await
    }

    async fn post_draft(
        &self,
        change_id: &str,
        payload: &CommentInput,
    ) -> Result<String, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/drafts"));

        #[derive(Deserialize)]
        struct DraftResponse {
            id: String,
        }

        let resp: DraftResponse = self.put_json(&url, payload).await?;
        Ok(resp.id)
    }

    async fn delete_draft(&self, change_id: &str, draft_id: &str) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let did = Self::percent_encode(draft_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/drafts/{did}"));
        self.delete_empty(&url).await
    }

    async fn publish_drafts(
        &self,
        change_id: &str,
        payload: &PublishDraftsRequest,
    ) -> Result<(), DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/revisions/current/review"));
        // Gerrit always returns a ReviewResult; parse it to validate the
        // response instead of silently ignoring the body.
        let _result: ReviewResult = self.post_json(&url, payload).await?;
        Ok(())
    }

    async fn cherry_pick(
        &self,
        change_id: &str,
        revision: &str,
        payload: &CherryPickRequest,
    ) -> Result<CherryPickResult, DomainError> {
        let cid = Self::percent_encode(change_id);
        let rev = Self::percent_encode(revision);
        let url = self.url(&format!("/changes/{cid}/revisions/{rev}/cherrypick"));
        self.post_json(&url, payload).await
    }

    async fn get_related(
        &self,
        change_id: &str,
        revision: &str,
    ) -> Result<Vec<RelatedChange>, DomainError> {
        let cid = Self::percent_encode(change_id);
        let rev = Self::percent_encode(revision);
        let url = self.url(&format!("/changes/{cid}/revisions/{rev}/related"));

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RelatedResponse {
            #[serde(default)]
            changes: Vec<RelatedChange>,
        }

        let resp: RelatedResponse = self.get_json(&url).await?;
        Ok(resp.changes)
    }

    async fn submit_change(
        &self,
        change_id: &str,
        payload: &SubmitRequest,
    ) -> Result<SubmitResult, DomainError> {
        let cid = Self::percent_encode(change_id);
        let url = self.url(&format!("/changes/{cid}/submit"));
        self.post_json(&url, payload).await
    }
}

// ---------------------------------------------------------------------------
// Helper for PUT + JSON response
// ---------------------------------------------------------------------------

impl GerritClient {
    async fn put_json<T: DeserializeOwned>(
        &self,
        url: &str,
        body: &impl Serialize,
    ) -> Result<T, DomainError> {
        let response = self.put_builder(url, body).send().await?;
        let response = Self::check_response(response).await?;
        let text = response.text().await?;
        let trimmed = Self::strip_xssi(&text);

        serde_json::from_str(trimmed).map_err(|e| {
            let truncated: String = text.chars().take(500).collect();
            DomainError::Decode(format!(
                "JSON parse error: {e}\nRaw body (500 chars): {truncated}"
            ))
        })
    }

    /// Send PUT, return the raw (XSSI-stripped) response text, which may be empty.
    async fn put_text(&self, url: &str, body: &impl Serialize) -> Result<String, DomainError> {
        let response = self.put_builder(url, body).send().await?;
        let response = Self::check_response(response).await?;
        let text = response.text().await?;
        Ok(Self::strip_xssi(&text).to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const XSSI_JSON: &str = ")]}'\n";

    fn test_client(base_url: &str) -> GerritClient {
        let tls_config = tls::build_tls_connector(&TlsConfig {
            verify_ssl: false,
            ..Default::default()
        })
        .unwrap();
        let http = Client::builder()
            .timeout(Duration::from_secs(5))
            .use_preconfigured_tls(tls_config)
            .build()
            .unwrap();
        GerritClient {
            client: http,
            base_url: base_url.to_string(),
            auth: AuthMode::Bearer("test-token".into()),
        }
    }

    // -- wiremock integration tests ----------------------------------------

    #[tokio::test]
    async fn test_query_changes_strips_xssi() {
        let server = MockServer::start().await;

        let change_json = r#"{"id":"project~branch~12345","_number":12345,"subject":"Test change","status":"NEW","project":"project","branch":"main","owner":{"_account_id":1000,"name":"Author","email":"author@example.com"},"updated":"2025-01-01 00:00:00"}"#;
        let body = format!("{XSSI_JSON}[{change_json}]");

        Mock::given(method("GET"))
            .and(path("/a/changes/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = test_client(&format!("{}/a", server.uri()));

        let result = client
            .query_changes("status:open", None, &[])
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]._number, 12345);
        assert_eq!(result[0].subject, "Test change");
    }

    #[tokio::test]
    async fn test_http_error_returns_domain_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/a/changes/"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;

        let client = test_client(&format!("{}/a", server.uri()));

        let result = client.query_changes("status:open", None, &[]).await;

        match result {
            Err(DomainError::HttpStatus { status, body }) => {
                assert_eq!(status, 404);
                assert!(body.contains("Not Found"));
            }
            other => panic!("expected HttpStatus error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_changes_submitted_together_bare_array() {
        let server = MockServer::start().await;

        let change_json = r#"{"id":"project~branch~12345","_number":12345,"subject":"Test change","status":"NEW","project":"project","branch":"main","owner":{"_account_id":1000},"updated":"2025-01-01 00:00:00"}"#;
        let body = format!("{XSSI_JSON}[{change_json}]");

        Mock::given(method("GET"))
            .and(path("/a/changes/35250/submitted_together"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = test_client(&format!("{}/a", server.uri()));

        let result = client
            .changes_submitted_together("35250", &[])
            .await
            .unwrap();

        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0]._number, 12345);
        assert_eq!(result.non_visible_changes, 0);
    }

    #[tokio::test]
    async fn test_changes_submitted_together_wrapped_object() {
        let server = MockServer::start().await;

        let change_json = r#"{"id":"project~branch~12345","_number":12345,"subject":"Test change","status":"NEW","project":"project","branch":"main","owner":{"_account_id":1000},"updated":"2025-01-01 00:00:00"}"#;
        let body = format!("{XSSI_JSON}{{\"changes\":[{change_json}],\"non_visible_changes\":2}}");

        Mock::given(method("GET"))
            .and(path("/a/changes/35250/submitted_together"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = test_client(&format!("{}/a", server.uri()));

        let result = client
            .changes_submitted_together("35250", &[])
            .await
            .unwrap();

        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.non_visible_changes, 2);
    }

    // -- percent_encode ----------------------------------------------------

    #[test]
    fn percent_encode_handles_spaces() {
        let encoded = GerritClient::percent_encode("hello world");
        assert!(encoded.contains("%20") || encoded.contains("+"));
    }

    #[test]
    fn percent_encode_handles_special_chars() {
        let encoded = GerritClient::percent_encode("project~branch~change-Id");
        assert!(encoded.contains("project"));
        assert!(encoded.contains("-"));
    }

    // -- build_options_query -----------------------------------------------

    #[test]
    fn build_options_query_empty() {
        let result = GerritClient::build_options_query(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn build_options_query_multiple() {
        let result = GerritClient::build_options_query(&[
            "CURRENT_REVISION".into(),
            "DETAILED_ACCOUNTS".into(),
        ]);
        assert!(
            result.starts_with("?o="),
            "should start with ?o=, got: {result}"
        );
        assert!(
            result.contains("?o=CURRENT_REVISION"),
            "should contain first option"
        );
        assert!(
            result.contains("&o=DETAILED_ACCOUNTS"),
            "should contain second option"
        );
    }

    // -- strip_xssi --------------------------------------------------------

    #[test]
    fn strip_xssi_removes_prefix() {
        let result = GerritClient::strip_xssi(")]}'\n{\"key\":\"value\"}");
        assert_eq!(result, "{\"key\":\"value\"}");
    }

    #[test]
    fn strip_xssi_no_prefix_passthrough() {
        let result = GerritClient::strip_xssi("{\"key\":\"value\"}");
        assert_eq!(result, "{\"key\":\"value\"}");
    }

    #[test]
    fn strip_xssi_empty_string() {
        let result = GerritClient::strip_xssi("");
        assert_eq!(result, "");
    }

    // -- url construction --------------------------------------------------

    #[test]
    fn url_joins_path() {
        let client = GerritClient {
            client: Client::builder().build().unwrap(),
            base_url: "https://gerrit.example.com/a".into(),
            auth: AuthMode::Bearer("token".into()),
        };
        let url = client.url("/changes/123");
        assert_eq!(url, "https://gerrit.example.com/a/changes/123");
    }

    // -- get_diff JSON DiffInfo parsing ---------------------------------

    #[tokio::test]
    async fn test_get_diff_parses_diff_info_json() {
        let server = MockServer::start().await;

        let line1 = "diff --git a/src/main.rs b/src/main.rs\n";
        let line2 = "+fn main() {}\n";
        use base64::Engine as _;
        let b64_line1 = base64::engine::general_purpose::STANDARD.encode(line1);
        let b64_line2 = base64::engine::general_purpose::STANDARD.encode(line2);

        let diff_info_json = format!(
            r#"{{"content":[{{"ab":["{}","{}"]}}]}}"#,
            b64_line1, b64_line2
        );

        Mock::given(method("GET"))
            .and(path("/changes/12345/revisions/current/patch"))
            .respond_with(ResponseTemplate::new(200).set_body_string(diff_info_json))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let result = client.get_diff("12345", "src/main.rs").await.unwrap();
        assert_eq!(result, format!("{}{}", line1, line2));
    }

    #[tokio::test]
    async fn test_get_diff_strips_xssi_prefix() {
        let server = MockServer::start().await;

        let line1 = "--- a/src/lib.rs\n+++ b/src/lib.rs\n";
        use base64::Engine as _;
        let b64_line1 = base64::engine::general_purpose::STANDARD.encode(line1);

        let diff_info_json = format!(r#"{{"content":[{{"ab":["{}"]}}]}}"#, b64_line1);
        let body = format!("{XSSI_JSON}{diff_info_json}");

        Mock::given(method("GET"))
            .and(path("/changes/xssi/revisions/current/patch"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let result = client.get_diff("xssi", "src/lib.rs").await.unwrap();
        assert_eq!(result, line1);
    }

    #[tokio::test]
    async fn test_set_labels_posts_review_input() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/changes/123/revisions/current/review"))
            .and(body_partial_json(
                serde_json::json!({"labels": {"READY-FOR-CI": 1}, "message": "Trigger CI"}),
            ))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = ReviewInput {
            message: Some("Trigger CI".into()),
            labels: Some(BTreeMap::from([("READY-FOR-CI".into(), 1)])),
            ..ReviewInput::default()
        };
        client.set_labels("123", &payload).await.unwrap();
    }

    #[tokio::test]
    async fn test_publish_drafts_posts_review_with_drafts_publish() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/changes/123/revisions/current/review"))
            .and(body_partial_json(serde_json::json!({
                "drafts": "PUBLISH_ALL_REVISIONS",
                "message": "Addressed all comments",
                "labels": {"Code-Review": 1},
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "labels": {"Code-Review": {"all": []}},
                "comments": {},
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = PublishDraftsRequest {
            drafts: DraftHandling::PublishAllRevisions,
            message: Some("Addressed all comments".into()),
            labels: Some(BTreeMap::from([("Code-Review".into(), 1)])),
        };
        client.publish_drafts("123", &payload).await.unwrap();
    }

    #[tokio::test]
    async fn test_post_draft_sends_range_and_unresolved() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/changes/123/revisions/current/drafts"))
            .and(body_partial_json(serde_json::json!({
                "path": "src/lib.rs",
                "message": "look at this",
                "range": {"startLine": 10, "startCharacter": 0, "endLine": 12, "endCharacter": 4},
                "unresolved": true,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(")]}'\n{\"id\":\"draft-1\"}"))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = CommentInput {
            id: None,
            path: Some("src/lib.rs".into()),
            side: None,
            line: None,
            range: Some(CommentRange {
                start_line: 10,
                start_character: 0,
                end_line: 12,
                end_character: 4,
            }),
            in_reply_to: None,
            updated: None,
            message: "look at this".into(),
            tag: None,
            unresolved: Some(true),
        };

        let draft_id = client.post_draft("123", &payload).await.unwrap();
        assert_eq!(draft_id, "draft-1");
    }

    #[tokio::test]
    async fn test_post_review_sends_labels() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/changes/123/revisions/current/review"))
            .and(body_partial_json(serde_json::json!({
                "comments": {
                    "src/lib.rs": [{
                        "path": "src/lib.rs",
                        "line": 5,
                        "message": "nit",
                        "unresolved": true,
                    }]
                },
                "labels": {"Code-Review": -1},
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let comment = CommentInput {
            id: None,
            path: Some("src/lib.rs".into()),
            side: None,
            line: Some(5),
            range: None,
            in_reply_to: None,
            updated: None,
            message: "nit".into(),
            tag: None,
            unresolved: Some(true),
        };
        let batch = CommentBatchInput {
            comments: Some(BTreeMap::from([("src/lib.rs".into(), vec![comment])])),
            omit_duplicate_comments: None,
            notify: None,
            labels: Some(BTreeMap::from([("Code-Review".into(), -1)])),
        };

        client.post_review("123", &batch).await.unwrap();
    }

    #[tokio::test]
    async fn test_set_topic_empty_body_returns_none() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/changes/123/topic"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = TopicRequest {
            topic: String::new(),
        };
        let result = client.set_topic("123", &payload).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cherry_pick_sends_flags() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/changes/123/revisions/1/cherrypick"))
            .and(body_partial_json(serde_json::json!({
                "destination": "main",
                "keepReviewers": true,
                "allowConflicts": true,
                "allowEmpty": true,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "new~100",
                "_number": 100,
                "subject": "Cp"
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = CherryPickRequest {
            message: None,
            destination: "main".into(),
            parent: None,
            base: None,
            notify: None,
            keep_reviewers: Some(true),
            allow_conflicts: Some(true),
            allow_empty: Some(true),
        };
        let result = client.cherry_pick("123", "1", &payload).await.unwrap();
        assert_eq!(result._number, 100);
    }

    #[tokio::test]
    async fn test_set_topic_parses_json_string() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/changes/123/topic"))
            .respond_with(ResponseTemplate::new(200).set_body_string(")]}'\n\"mytopic\""))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());

        let payload = TopicRequest {
            topic: "mytopic".into(),
        };
        let result = client.set_topic("123", &payload).await.unwrap();
        assert_eq!(result, Some("mytopic".to_string()));
    }

    #[tokio::test]
    async fn test_get_commit_message_uses_message_endpoint() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/changes/123/message"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "full_message": "Fix stuff\n\nDetails here\n\nChange-Id: Iabc123"
            })))
            .mount(&server)
            .await;

        let client = test_client(&server.uri());
        let result = client.get_commit_message("123").await.unwrap();
        assert_eq!(
            result.full_message,
            "Fix stuff\n\nDetails here\n\nChange-Id: Iabc123"
        );
    }
}
