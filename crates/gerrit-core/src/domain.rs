// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Domain models, errors, and traits for Gerrit code review.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level error type for the Gerrit core library.
#[derive(Error, Debug, Clone)]
pub enum DomainError {
    #[error("query string must not be empty")]
    EmptyQuery,

    #[error("invalid change ID format")]
    InvalidChangeId,

    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("JSON decode error: {0}")]
    Decode(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error(transparent)]
    Cache(#[from] CacheError),

    #[error(transparent)]
    RateLimit(#[from] RateLimitError),

    #[error("not implemented")]
    NotImplemented,
}

impl From<reqwest::Error> for DomainError {
    fn from(e: reqwest::Error) -> Self {
        DomainError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(e: serde_json::Error) -> Self {
        DomainError::Decode(e.to_string())
    }
}

/// Cache-related errors.
#[derive(Error, Debug, Clone)]
pub enum CacheError {
    #[error("cache capacity exceeded")]
    CapacityExceeded,
}

/// Rate-limit errors.
#[derive(Error, Debug, Clone)]
pub enum RateLimitError {
    #[error("rate limit exceeded; retry after {retry_after_secs}s")]
    Exceeded { retry_after_secs: u64 },
}

// ---------------------------------------------------------------------------
// Shared response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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
}

// ---------------------------------------------------------------------------
// ChangeDetail
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
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
}

// ---------------------------------------------------------------------------
// RevisionInfo / CommitWithMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionInfo {
    #[serde(rename = "_number")]
    pub _number: u64,
    #[serde(default)]
    pub commit: Option<CommitWithMessage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitWithMessage {
    pub message: String,
}

// ---------------------------------------------------------------------------
// LabelInfo / VoteInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelInfo {
    #[serde(default)]
    pub all: Vec<VoteInfo>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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
    pub full_message: String,
    #[serde(default)]
    pub footers: BTreeMap<String, String>,
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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
}
