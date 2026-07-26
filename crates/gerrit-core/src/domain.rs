// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Domain models, errors, and traits for Gerrit code review.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

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
// MockGerritRepository
// ---------------------------------------------------------------------------

/// In-memory mock for testing consumers of the GerritRepository trait.
///
/// Push expected results via `push_X` methods before calling the corresponding
/// trait method. Each call pops one result from the queue.
#[allow(clippy::type_complexity)]
#[derive(Default)]
pub struct MockGerritRepository {
    pub query_changes_results: Mutex<Vec<Result<Vec<Change>, DomainError>>>,
    pub query_changes_call_count: AtomicUsize,
    pub last_query: RwLock<Option<QueryParams>>,

    pub get_change_detail_results: Mutex<Vec<Result<ChangeDetail, DomainError>>>,
    pub get_commit_message_results: Mutex<Vec<Result<CommitMessage, DomainError>>>,
    pub list_files_results: Mutex<Vec<Result<BTreeMap<String, FileInfo>, DomainError>>>,
    pub get_diff_results: Mutex<Vec<Result<String, DomainError>>>,
    pub list_comments_results: Mutex<Vec<Result<BTreeMap<String, Vec<Comment>>, DomainError>>>,
    pub list_drafts_results: Mutex<Vec<Result<BTreeMap<String, Vec<DraftComment>>, DomainError>>>,
    pub get_commit_results: Mutex<Vec<Result<CommitInfo, DomainError>>>,
    pub suggest_reviewers_results: Mutex<Vec<Result<Vec<SuggestedReviewer>, DomainError>>>,
    pub changes_submitted_together_results: Mutex<Vec<Result<SubmittedTogether, DomainError>>>,
    pub create_change_results: Mutex<Vec<Result<Change, DomainError>>>,
    pub add_reviewer_results: Mutex<Vec<Result<AddReviewerResult, DomainError>>>,
    pub set_ready_results: Mutex<Vec<Result<(), DomainError>>>,
    pub set_wip_results: Mutex<Vec<Result<(), DomainError>>>,
    pub set_topic_results: Mutex<Vec<Result<Option<String>, DomainError>>>,
    pub abandon_change_results: Mutex<Vec<Result<Change, DomainError>>>,
    pub revert_change_results: Mutex<Vec<Result<Change, DomainError>>>,
    pub revert_submission_results: Mutex<Vec<Result<Vec<Change>, DomainError>>>,
    pub post_review_results: Mutex<Vec<Result<(), DomainError>>>,
    pub post_draft_results: Mutex<Vec<Result<String, DomainError>>>,
    pub delete_draft_results: Mutex<Vec<Result<(), DomainError>>>,
    pub publish_drafts_results: Mutex<Vec<Result<(), DomainError>>>,
    pub cherry_pick_results: Mutex<Vec<Result<CherryPickResult, DomainError>>>,
    pub get_related_results: Mutex<Vec<Result<Vec<RelatedChange>, DomainError>>>,
    pub submit_change_results: Mutex<Vec<Result<SubmitResult, DomainError>>>,
}

impl MockGerritRepository {
    pub fn push_query_changes_result(&self, result: Result<Vec<Change>, DomainError>) {
        self.query_changes_results.lock().unwrap().push(result);
    }

    pub fn push_get_change_detail_result(&self, result: Result<ChangeDetail, DomainError>) {
        self.get_change_detail_results.lock().unwrap().push(result);
    }

    pub fn push_get_commit_message_result(&self, result: Result<CommitMessage, DomainError>) {
        self.get_commit_message_results.lock().unwrap().push(result);
    }

    pub fn push_list_files_result(&self, result: Result<BTreeMap<String, FileInfo>, DomainError>) {
        self.list_files_results.lock().unwrap().push(result);
    }

    pub fn push_get_diff_result(&self, result: Result<String, DomainError>) {
        self.get_diff_results.lock().unwrap().push(result);
    }

    pub fn push_list_comments_result(
        &self,
        result: Result<BTreeMap<String, Vec<Comment>>, DomainError>,
    ) {
        self.list_comments_results.lock().unwrap().push(result);
    }

    pub fn push_list_drafts_result(
        &self,
        result: Result<BTreeMap<String, Vec<DraftComment>>, DomainError>,
    ) {
        self.list_drafts_results.lock().unwrap().push(result);
    }

    pub fn push_get_commit_result(&self, result: Result<CommitInfo, DomainError>) {
        self.get_commit_results.lock().unwrap().push(result);
    }

    pub fn push_suggest_reviewers_result(
        &self,
        result: Result<Vec<SuggestedReviewer>, DomainError>,
    ) {
        self.suggest_reviewers_results.lock().unwrap().push(result);
    }

    pub fn push_changes_submitted_together_result(
        &self,
        result: Result<SubmittedTogether, DomainError>,
    ) {
        self.changes_submitted_together_results
            .lock()
            .unwrap()
            .push(result);
    }

    pub fn push_create_change_result(&self, result: Result<Change, DomainError>) {
        self.create_change_results.lock().unwrap().push(result);
    }

    pub fn push_add_reviewer_result(&self, result: Result<AddReviewerResult, DomainError>) {
        self.add_reviewer_results.lock().unwrap().push(result);
    }

    pub fn push_set_ready_result(&self, result: Result<(), DomainError>) {
        self.set_ready_results.lock().unwrap().push(result);
    }

    pub fn push_set_wip_result(&self, result: Result<(), DomainError>) {
        self.set_wip_results.lock().unwrap().push(result);
    }

    pub fn push_set_topic_result(&self, result: Result<Option<String>, DomainError>) {
        self.set_topic_results.lock().unwrap().push(result);
    }

    pub fn push_abandon_change_result(&self, result: Result<Change, DomainError>) {
        self.abandon_change_results.lock().unwrap().push(result);
    }

    pub fn push_revert_change_result(&self, result: Result<Change, DomainError>) {
        self.revert_change_results.lock().unwrap().push(result);
    }

    pub fn push_revert_submission_result(&self, result: Result<Vec<Change>, DomainError>) {
        self.revert_submission_results.lock().unwrap().push(result);
    }

    pub fn push_post_review_result(&self, result: Result<(), DomainError>) {
        self.post_review_results.lock().unwrap().push(result);
    }

    pub fn push_post_draft_result(&self, result: Result<String, DomainError>) {
        self.post_draft_results.lock().unwrap().push(result);
    }

    pub fn push_delete_draft_result(&self, result: Result<(), DomainError>) {
        self.delete_draft_results.lock().unwrap().push(result);
    }

    pub fn push_publish_drafts_result(&self, result: Result<(), DomainError>) {
        self.publish_drafts_results.lock().unwrap().push(result);
    }

    pub fn push_cherry_pick_result(&self, result: Result<CherryPickResult, DomainError>) {
        self.cherry_pick_results.lock().unwrap().push(result);
    }

    pub fn push_get_related_result(&self, result: Result<Vec<RelatedChange>, DomainError>) {
        self.get_related_results.lock().unwrap().push(result);
    }

    pub fn push_submit_change_result(&self, result: Result<SubmitResult, DomainError>) {
        self.submit_change_results.lock().unwrap().push(result);
    }

    pub fn query_changes_call_count(&self) -> usize {
        self.query_changes_call_count.load(Ordering::SeqCst)
    }

    pub fn last_query(&self) -> Option<QueryParams> {
        self.last_query.read().unwrap().clone()
    }
}

fn not_implemented<T>() -> Result<T, DomainError> {
    Err(DomainError::NotImplemented)
}

macro_rules! pop_result {
    ($field:expr) => {{
        $field
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| not_implemented())
    }};
}

#[async_trait::async_trait]
impl GerritRepository for MockGerritRepository {
    async fn query_changes(
        &self,
        query: &str,
        limit: Option<u32>,
        options: &[String],
    ) -> Result<Vec<Change>, DomainError> {
        self.query_changes_call_count.fetch_add(1, Ordering::SeqCst);
        *self.last_query.write().unwrap() = Some(QueryParams {
            query: query.to_string(),
            limit,
            options: options.to_vec(),
        });
        pop_result!(self.query_changes_results)
    }

    async fn get_change_detail(
        &self,
        _change_id: &str,
        _options: &[String],
    ) -> Result<ChangeDetail, DomainError> {
        pop_result!(self.get_change_detail_results)
    }

    async fn get_commit_message(&self, _change_id: &str) -> Result<CommitMessage, DomainError> {
        pop_result!(self.get_commit_message_results)
    }

    async fn list_files(
        &self,
        _change_id: &str,
    ) -> Result<BTreeMap<String, FileInfo>, DomainError> {
        pop_result!(self.list_files_results)
    }

    async fn get_diff(&self, _change_id: &str, _file_path: &str) -> Result<String, DomainError> {
        pop_result!(self.get_diff_results)
    }

    async fn list_comments(
        &self,
        _change_id: &str,
    ) -> Result<BTreeMap<String, Vec<Comment>>, DomainError> {
        pop_result!(self.list_comments_results)
    }

    async fn list_drafts(
        &self,
        _change_id: &str,
    ) -> Result<BTreeMap<String, Vec<DraftComment>>, DomainError> {
        pop_result!(self.list_drafts_results)
    }

    async fn get_commit(&self, _change_id: &str) -> Result<CommitInfo, DomainError> {
        pop_result!(self.get_commit_results)
    }

    async fn suggest_reviewers(
        &self,
        _change_id: &str,
        _query: &str,
        _limit: Option<u32>,
        _exclude_groups: bool,
        _reviewer_state: Option<&str>,
    ) -> Result<Vec<SuggestedReviewer>, DomainError> {
        pop_result!(self.suggest_reviewers_results)
    }

    async fn changes_submitted_together(
        &self,
        _change_id: &str,
        _options: &[String],
    ) -> Result<SubmittedTogether, DomainError> {
        pop_result!(self.changes_submitted_together_results)
    }

    async fn create_change(&self, _payload: &CreateChangeRequest) -> Result<Change, DomainError> {
        pop_result!(self.create_change_results)
    }

    async fn add_reviewer(
        &self,
        _change_id: &str,
        _payload: &AddReviewerRequest,
    ) -> Result<AddReviewerResult, DomainError> {
        pop_result!(self.add_reviewer_results)
    }

    async fn set_ready(&self, _change_id: &str) -> Result<(), DomainError> {
        pop_result!(self.set_ready_results)
    }

    async fn set_wip(&self, _change_id: &str, _payload: &WipRequest) -> Result<(), DomainError> {
        pop_result!(self.set_wip_results)
    }

    async fn set_topic(
        &self,
        _change_id: &str,
        _payload: &TopicRequest,
    ) -> Result<Option<String>, DomainError> {
        pop_result!(self.set_topic_results)
    }

    async fn abandon_change(
        &self,
        _change_id: &str,
        _payload: &AbandonRequest,
    ) -> Result<Change, DomainError> {
        pop_result!(self.abandon_change_results)
    }

    async fn revert_change(
        &self,
        _change_id: &str,
        _message: Option<&str>,
    ) -> Result<Change, DomainError> {
        pop_result!(self.revert_change_results)
    }

    async fn revert_submission(
        &self,
        _change_id: &str,
        _message: Option<&str>,
    ) -> Result<Vec<Change>, DomainError> {
        pop_result!(self.revert_submission_results)
    }

    async fn post_review(
        &self,
        _change_id: &str,
        _payload: &CommentBatchInput,
    ) -> Result<(), DomainError> {
        pop_result!(self.post_review_results)
    }

    async fn post_draft(
        &self,
        _change_id: &str,
        _payload: &DraftInput,
    ) -> Result<String, DomainError> {
        pop_result!(self.post_draft_results)
    }

    async fn delete_draft(&self, _change_id: &str, _draft_id: &str) -> Result<(), DomainError> {
        pop_result!(self.delete_draft_results)
    }

    async fn publish_drafts(
        &self,
        _change_id: &str,
        _payload: &PublishDraftsRequest,
    ) -> Result<(), DomainError> {
        pop_result!(self.publish_drafts_results)
    }

    async fn cherry_pick(
        &self,
        _change_id: &str,
        _revision: &str,
        _payload: &CherryPickRequest,
    ) -> Result<CherryPickResult, DomainError> {
        pop_result!(self.cherry_pick_results)
    }

    async fn get_related(
        &self,
        _change_id: &str,
        _revision: &str,
    ) -> Result<Vec<RelatedChange>, DomainError> {
        pop_result!(self.get_related_results)
    }

    async fn submit_change(
        &self,
        _change_id: &str,
        _payload: &SubmitRequest,
    ) -> Result<SubmitResult, DomainError> {
        pop_result!(self.submit_change_results)
    }
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
    // MockGerritRepository tests
    // -----------------------------------------------------------------------

    fn make_change(number: u64, subject: &str) -> Change {
        Change {
            id: format!("project~branch~{}", number),
            _number: number,
            subject: subject.to_string(),
            status: "NEW".to_string(),
            project: "project".to_string(),
            branch: "main".to_string(),
            owner: AccountInfo {
                _account_id: 1000,
                name: Some("Author".into()),
                email: Some("author@example.com".into()),
            },
            updated: "2025-01-01 00:00:00".into(),
            work_in_progress: false,
        }
    }

    #[tokio::test]
    async fn mock_query_changes_returns_pushed_result() {
        let mock = MockGerritRepository::default();
        let expected = make_change(12345, "Test change");
        mock.push_query_changes_result(Ok(vec![expected.clone()]));

        let result = mock
            .query_changes("status:open", Some(10), &[])
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]._number, 12345);
        assert_eq!(mock.query_changes_call_count(), 1);
    }

    #[tokio::test]
    async fn mock_query_changes_returns_error() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Err(DomainError::EmptyQuery));

        let result = mock.query_changes("", None, &[]).await;
        assert!(result.is_err());
        assert_eq!(mock.query_changes_call_count(), 1);
    }

    #[tokio::test]
    async fn mock_tracks_last_query() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![]));

        let _ = mock
            .query_changes("status:open", Some(5), &["DETAILED_ACCOUNTS".into()])
            .await;

        let params = mock.last_query().unwrap();
        assert_eq!(params.query, "status:open");
        assert_eq!(params.limit, Some(5));
        assert_eq!(params.options, vec!["DETAILED_ACCOUNTS"]);
    }

    #[tokio::test]
    async fn mock_get_change_detail_returns_pushed() {
        let mock = MockGerritRepository::default();
        mock.push_get_change_detail_result(Ok(ChangeDetail {
            id: "test~123".into(),
            _number: 456,
            subject: "Detail test".into(),
            status: "NEW".into(),
            project: "p".into(),
            branch: "b".into(),
            owner: AccountInfo {
                _account_id: 1,
                name: None,
                email: None,
            },
            updated: "now".into(),
            current_revision: None,
            current_revision_number: None,
            revisions: BTreeMap::new(),
            labels: BTreeMap::new(),
            reviewers: None,
            messages: vec![],
        }));

        let detail = mock.get_change_detail("test~123", &[]).await.unwrap();
        assert_eq!(detail._number, 456);
    }

    #[tokio::test]
    async fn mock_returns_not_implemented_when_empty() {
        let mock = MockGerritRepository::default();
        let err = mock.get_change_detail("any", &[]).await.unwrap_err();
        match err {
            DomainError::NotImplemented => {}
            _ => panic!("expected NotImplemented"),
        }
    }

    #[tokio::test]
    async fn mock_cherry_pick_returns_pushed() {
        let mock = MockGerritRepository::default();
        mock.push_cherry_pick_result(Ok(CherryPickResult {
            id: "new~999".into(),
            _number: 999,
            subject: "Cherry-picked".into(),
        }));

        let result = mock
            .cherry_pick(
                "src~1",
                "1",
                &CherryPickRequest {
                    message: None,
                    destination: "main".into(),
                    parent: None,
                    base: None,
                    notify: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result._number, 999);
    }

    #[tokio::test]
    async fn mock_submit_returns_pushed() {
        let mock = MockGerritRepository::default();
        mock.push_submit_change_result(Ok(SubmitResult {
            id: "test~42".into(),
            _number: 42,
            subject: "Merged change".into(),
            status: "MERGED".into(),
        }));

        let result = mock
            .submit_change(
                "test~42",
                &SubmitRequest {
                    wait_for_merge: None,
                    on_behalf_of: None,
                    notify: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.status, "MERGED");
    }
}
