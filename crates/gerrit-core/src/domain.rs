// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Domain models, errors, and traits for Gerrit code review.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod error;
pub use error::*;
pub mod mock;
pub use mock::MockGerritRepository;

// ---------------------------------------------------------------------------
// Shared response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    #[serde(rename = "_account_id")]
    pub _account_id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Change
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    #[serde(rename = "_number")]
    pub _number: u64,
    pub subject: String,
    pub status: String,
    pub project: String,
    pub branch: String,
    pub owner: AccountInfo,
    pub updated: String,
    #[serde(default)]
    pub work_in_progress: bool,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub reviewers: Option<BTreeMap<String, Vec<ReviewerInfo>>>,
}

// ---------------------------------------------------------------------------
// ChangeDetail
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDetail {
    pub id: String,
    #[serde(rename = "_number")]
    pub _number: u64,
    pub subject: String,
    pub status: String,
    pub project: String,
    pub branch: String,
    pub owner: AccountInfo,
    pub updated: String,
    #[serde(default)]
    pub current_revision: Option<String>,
    #[serde(default)]
    pub current_revision_number: Option<u64>,
    #[serde(default)]
    pub revisions: BTreeMap<String, RevisionInfo>,
    #[serde(default)]
    pub labels: BTreeMap<String, LabelInfo>,
    #[serde(default)]
    pub reviewers: Option<BTreeMap<String, Vec<ReviewerInfo>>>,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub topic: Option<String>,
}

// ---------------------------------------------------------------------------
// RevisionInfo / CommitWithMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionInfo {
    #[serde(rename = "_number")]
    pub _number: u64,
    #[serde(default)]
    pub commit: Option<CommitWithMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitWithMessage {
    pub message: String,
}

// ---------------------------------------------------------------------------
// LabelInfo / VoteInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelInfo {
    #[serde(default)]
    pub all: Vec<VoteInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteInfo {
    #[serde(rename = "_account_id")]
    pub _account_id: u64,
    #[serde(default)]
    pub value: Option<i32>,
}

// ---------------------------------------------------------------------------
// ReviewerInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerInfo {
    #[serde(rename = "_account_id")]
    pub _account_id: u64,
    #[serde(default)]
    pub email: Option<String>,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(default)]
    pub author: Option<AccountInfo>,
    pub date: String,
    pub message: String,
    #[serde(rename = "_revision_number")]
    pub _revision_number: u64,
}

// ---------------------------------------------------------------------------
// CommitMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessage {
    pub subject: String,
    pub message: String,
    pub commit: String,
    pub author: GitPersonInfo,
    pub committer: GitPersonInfo,
    #[serde(default)]
    pub parents: Vec<CommitParent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPersonInfo {
    pub name: String,
    pub email: String,
    pub date: String,
    #[serde(default)]
    pub tz: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitParent {
    pub commit: String,
    #[serde(default)]
    pub subject: Option<String>,
}

// ---------------------------------------------------------------------------
// FileInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub lines_inserted: i64,
    #[serde(default)]
    pub lines_deleted: i64,
}

// ---------------------------------------------------------------------------
// Comment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub line: Option<u64>,
    pub message: String,
    #[serde(default)]
    pub author: Option<AccountInfo>,
    pub updated: String,
    #[serde(default)]
    pub unresolved: Option<bool>,
}

// ---------------------------------------------------------------------------
// DraftComment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftComment {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    pub message: String,
}

// ---------------------------------------------------------------------------
// SuggestedReviewer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedReviewer {
    #[serde(default)]
    pub account: Option<AccountInfo>,
    #[serde(default)]
    pub group: Option<GroupInfo>,
}

// ---------------------------------------------------------------------------
// RelatedChange / RelatedChanges
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedChange {
    #[serde(rename = "_change_number")]
    pub _change_number: u64,
    #[serde(rename = "_revision_number")]
    pub _revision_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedChanges {
    #[serde(default)]
    pub changes: Vec<RelatedChange>,
}

// ---------------------------------------------------------------------------
// SubmittedTogether
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmittedTogether {
    #[serde(default)]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub non_visible_changes: u64,
}

// ---------------------------------------------------------------------------
// CherryPickResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CherryPickResult {
    pub id: String,
    #[serde(rename = "_number")]
    pub _number: u64,
    pub subject: String,
}

// ---------------------------------------------------------------------------
// SubmitResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResult {
    pub id: String,
    #[serde(rename = "_number")]
    pub _number: u64,
    pub subject: String,
    pub status: String,
}

// ---------------------------------------------------------------------------
// RevertResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertResult {
    #[serde(rename = "_number")]
    pub _number: u64,
    pub subject: String,
}

// ---------------------------------------------------------------------------
// AddReviewerResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddReviewerResult {
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub reviewers: Vec<ReviewerInfo>,
}

// ---------------------------------------------------------------------------
// CommitInfo (for creating commits)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub message: String,
}

// ---------------------------------------------------------------------------
// QueryParams (for mock tracking / transport)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

// ---------------------------------------------------------------------------
// Request payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChangeRequest {
    pub project: String,
    pub branch: String,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_in_progress: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_change: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_branch: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<BTreeMap<String, Vec<CommentInput>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omit_duplicate_comments: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInput {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRange {
    #[serde(default)]
    pub start_line: u64,
    #[serde(default)]
    pub start_character: u64,
    #[serde(default)]
    pub end_line: u64,
    #[serde(default)]
    pub end_character: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CherryPickRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub destination: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddReviewerRequest {
    pub reviewer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicRequest {
    pub topic: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbandonRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WipRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_for_merge: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDraftsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentBatchInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<BTreeMap<String, Vec<CommentInput>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafts: Option<BTreeMap<String, Vec<DraftInput>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omit_duplicate_comments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<CommentRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<bool>,
}

// ---------------------------------------------------------------------------
// GerritRepository trait
// ---------------------------------------------------------------------------

/// Core trait abstracting all Gerrit API operations.
#[async_trait::async_trait]
pub trait GerritRepository: Send + Sync {
    async fn query_changes(
        &self,
        query: &str,
        limit: Option<u32>,
        options: &[String],
    ) -> Result<Vec<Change>, DomainError>;

    async fn get_change_detail(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<ChangeDetail, DomainError>;

    async fn get_commit_message(&self, change_id: &str) -> Result<CommitMessage, DomainError>;

    async fn list_files(&self, change_id: &str) -> Result<BTreeMap<String, FileInfo>, DomainError>;

    async fn get_diff(&self, change_id: &str, file_path: &str) -> Result<String, DomainError>;

    async fn list_comments(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<Comment>>, DomainError>;

    async fn list_drafts(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<DraftComment>>, DomainError>;

    async fn get_commit(&self, change_id: &str) -> Result<CommitInfo, DomainError>;

    async fn suggest_reviewers(
        &self,
        change_id: &str,
        query: &str,
        limit: Option<u32>,
        exclude_groups: bool,
        reviewer_state: Option<&str>,
    ) -> Result<Vec<SuggestedReviewer>, DomainError>;

    async fn changes_submitted_together(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<SubmittedTogether, DomainError>;

    async fn create_change(&self, payload: &CreateChangeRequest) -> Result<Change, DomainError>;

    async fn add_reviewer(
        &self,
        change_id: &str,
        payload: &AddReviewerRequest,
    ) -> Result<AddReviewerResult, DomainError>;

    async fn set_ready(&self, change_id: &str) -> Result<(), DomainError>;

    async fn set_wip(&self, change_id: &str, payload: &WipRequest) -> Result<(), DomainError>;

    async fn set_topic(
        &self,
        change_id: &str,
        payload: &TopicRequest,
    ) -> Result<Option<String>, DomainError>;

    async fn abandon_change(
        &self,
        change_id: &str,
        payload: &AbandonRequest,
    ) -> Result<Change, DomainError>;

    async fn revert_change(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Change, DomainError>;

    async fn revert_submission(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Vec<Change>, DomainError>;

    async fn post_review(
        &self,
        change_id: &str,
        payload: &CommentBatchInput,
    ) -> Result<(), DomainError>;

    async fn post_draft(
        &self,
        change_id: &str,
        payload: &DraftInput,
    ) -> Result<String, DomainError>;

    async fn delete_draft(&self, change_id: &str, draft_id: &str) -> Result<(), DomainError>;

    async fn publish_drafts(
        &self,
        change_id: &str,
        payload: &PublishDraftsRequest,
    ) -> Result<(), DomainError>;

    async fn cherry_pick(
        &self,
        change_id: &str,
        revision: &str,
        payload: &CherryPickRequest,
    ) -> Result<CherryPickResult, DomainError>;

    async fn get_related(
        &self,
        change_id: &str,
        revision: &str,
    ) -> Result<Vec<RelatedChange>, DomainError>;

    async fn submit_change(
        &self,
        change_id: &str,
        payload: &SubmitRequest,
    ) -> Result<SubmitResult, DomainError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_error_display_empty_query() {
        let err = DomainError::EmptyQuery;
        assert_eq!(err.to_string(), "query string must not be empty");
    }

    #[test]
    fn domain_error_http_status() {
        let err = DomainError::HttpStatus {
            status: 404,
            body: "Not Found".into(),
        };
        assert_eq!(err.to_string(), "HTTP 404: Not Found");
    }

    #[test]
    fn domain_error_tls() {
        let err = DomainError::Tls("bad certificate".into());
        assert_eq!(err.to_string(), "TLS error: bad certificate");
    }

    #[test]
    fn domain_error_auth() {
        let err = DomainError::Auth("invalid credentials".into());
        assert_eq!(err.to_string(), "authentication error: invalid credentials");
    }

    // -----------------------------------------------------------------------
    // Deserialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_change_deserialize_with_topic() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "Test change with topic",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00",
            "topic": "my-topic-name"
        }"#;

        let change: Change = serde_json::from_str(json).unwrap();
        assert_eq!(change._number, 12345);
        assert_eq!(change.topic, Some("my-topic-name".to_string()));
    }

    #[test]
    fn test_change_deserialize_without_topic() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "Test change without topic",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00"
        }"#;

        let change: Change = serde_json::from_str(json).unwrap();
        assert_eq!(change._number, 12345);
        assert_eq!(change.topic, None);
    }

    #[test]
    fn test_change_detail_deserialize_with_topic() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "Test detail with topic",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00",
            "topic": "feature-topic"
        }"#;

        let detail: ChangeDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail._number, 12345);
        assert_eq!(detail.topic, Some("feature-topic".to_string()));
    }

    #[test]
    fn test_change_detail_deserialize_without_topic() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "Test detail without topic",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00"
        }"#;

        let detail: ChangeDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail._number, 12345);
        assert_eq!(detail.topic, None);
    }

    #[test]
    fn test_change_deserialize_with_reviewers() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "Test with reviewers",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00",
            "reviewers": {
                "REVIEWER": [{"_account_id": 2000, "email": "rev@example.com"}],
                "CC": [{"_account_id": 3000, "email": "cc@example.com"}]
            }
        }"#;

        let change: Change = serde_json::from_str(json).unwrap();
        let reviewers = change.reviewers.as_ref().unwrap();
        let reviewer_list = reviewers.get("REVIEWER").unwrap();
        assert_eq!(reviewer_list.len(), 1);
        assert_eq!(reviewer_list[0]._account_id, 2000);
        assert_eq!(reviewer_list[0].email.as_deref(), Some("rev@example.com"));

        let cc_list = reviewers.get("CC").unwrap();
        assert_eq!(cc_list.len(), 1);
        assert_eq!(cc_list[0]._account_id, 3000);
        assert_eq!(cc_list[0].email.as_deref(), Some("cc@example.com"));
    }

    #[test]
    fn test_change_deserialize_without_reviewers() {
        let json = r#"{
            "id": "project~branch~12345",
            "_number": 12345,
            "subject": "No reviewers",
            "status": "NEW",
            "project": "my-project",
            "branch": "main",
            "owner": {"_account_id": 1000, "name": "Author", "email": "author@example.com"},
            "updated": "2025-01-01 00:00:00"
        }"#;

        let change: Change = serde_json::from_str(json).unwrap();
        assert_eq!(change.reviewers, None);
    }
}
