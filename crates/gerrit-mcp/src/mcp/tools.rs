// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! MCP tool parameter types (input schemas).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Default helper functions
// ---------------------------------------------------------------------------

fn default_merged_status() -> Option<String> {
    Some("merged".to_string())
}

fn default_false() -> Option<bool> {
    Some(false)
}

fn default_true() -> Option<bool> {
    Some(true)
}

fn default_reviewer_state() -> Option<String> {
    Some("REVIEWER".to_string())
}

fn default_current_revision() -> Option<String> {
    Some("current".to_string())
}

// ---------------------------------------------------------------------------
// NoParams
// ---------------------------------------------------------------------------

/// Empty parameters for tools that require no input.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Empty parameters for tools that require no input")]
pub struct NoParams {}

// ---------------------------------------------------------------------------
// Read tools (12)
// ---------------------------------------------------------------------------

/// Parameters for querying changes by Gerrit search query.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Query Gerrit changes using a search query string")]
pub struct QueryChangesParams {
    #[schemars(description = "Gerrit search query string (e.g. 'status:open project:myproject')")]
    pub query: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Maximum number of results to return")]
    pub limit: Option<u32>,

    #[serde(default)]
    #[schemars(
        default,
        description = "Additional Gerrit change options (e.g. DETAILED_ACCOUNTS, CURRENT_REVISION)"
    )]
    pub options: Option<Vec<String>>,
}

/// Parameters for querying changes by date range with optional filters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Query Gerrit changes within a date range")]
pub struct QueryChangesByDateParams {
    #[schemars(description = "Start date in YYYY-MM-DD format")]
    pub start_date: String,

    #[schemars(description = "End date in YYYY-MM-DD format")]
    pub end_date: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Maximum number of results to return")]
    pub limit: Option<u32>,

    #[serde(default)]
    #[schemars(default, description = "Gerrit project name filter")]
    pub project: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Substring to match in commit messages")]
    pub message_substring: Option<String>,

    #[serde(default = "default_merged_status")]
    #[schemars(default, description = "Change status filter (merged, abandoned, open)")]
    pub status: Option<String>,
}

/// Parameters for getting detailed information about a specific change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Get detailed information about a Gerrit change")]
pub struct GetChangeDetailsParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,

    #[serde(default)]
    #[schemars(
        default,
        description = "Additional Gerrit change options (e.g. DETAILED_ACCOUNTS, CURRENT_REVISION)"
    )]
    pub options: Option<Vec<String>>,
}

/// Parameters for getting the commit message of a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Get the commit message for a Gerrit change")]
pub struct GetCommitMessageParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for listing files modified by a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "List files modified in a Gerrit change")]
pub struct ListChangeFilesParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for getting the diff of a specific file in a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Get the diff for a file in a Gerrit change")]
pub struct GetFileDiffParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Path to the file within the change")]
    pub file_path: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for listing comments on a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "List published comments on a Gerrit change")]
pub struct ListChangeCommentsParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for listing draft comments on a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "List draft comments on a Gerrit change")]
pub struct ListDraftCommentsParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for getting the most recent CL for a user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Get the most recent change from a Gerrit user")]
pub struct GetMostRecentClParams {
    #[schemars(description = "Username or email of the Gerrit user")]
    pub user: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for extracting bugs from a change's description.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Extract bug references from a Gerrit change")]
pub struct GetBugsFromClParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for suggesting reviewers for a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Suggest reviewers for a Gerrit change")]
pub struct SuggestReviewersParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Search query to filter suggested reviewers")]
    pub query: String,

    #[serde(default)]
    #[schemars(default, description = "Maximum number of reviewer suggestions to return")]
    pub limit: Option<u32>,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to exclude group members from suggestions")]
    pub exclude_groups: Option<bool>,

    #[serde(default)]
    #[schemars(default, description = "Filter by reviewer state (REVIEWER, CC)")]
    pub reviewer_state: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for listing changes submitted together.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "List changes submitted together with this one")]
pub struct ChangesSubmittedTogetherParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,

    #[serde(default)]
    #[schemars(
        default,
        description = "Additional Gerrit change options (e.g. CURRENT_COMMIT, CURRENT_REVISION)"
    )]
    pub options: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Write tools (13)
// ---------------------------------------------------------------------------

/// Parameters for creating a new change in Gerrit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Create a new change in Gerrit")]
pub struct CreateChangeParams {
    #[schemars(description = "Gerrit project name")]
    pub project: String,

    #[schemars(description = "Subject line of the change")]
    pub subject: String,

    #[schemars(description = "Target branch name")]
    pub branch: String,

    #[serde(default)]
    #[schemars(default, description = "Topic for the change")]
    pub topic: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Status for the new change (NEW, DRAFT)")]
    pub status: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,
}

/// Parameters for adding a reviewer to a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Add a reviewer to a Gerrit change")]
pub struct AddReviewerParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Email or username of the reviewer to add")]
    pub reviewer: String,

    #[serde(default)]
    #[schemars(default, description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: Option<String>,

    #[serde(default = "default_reviewer_state")]
    #[schemars(default, description = "Reviewer state (REVIEWER, CC)")]
    pub state: Option<String>,
}

/// Parameters for marking a change as ready for review.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Mark a Gerrit change as ready for review")]
pub struct SetReadyParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for setting a change as work-in-progress.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Mark a Gerrit change as work-in-progress")]
pub struct SetWipParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Optional message explaining why the change is WIP")]
    pub message: Option<String>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for setting the topic of a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Set the topic for a Gerrit change")]
pub struct SetTopicParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Topic to set on the change")]
    pub topic: String,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for abandoning a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Abandon a Gerrit change")]
pub struct AbandonChangeParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Optional message explaining the reason for abandonment")]
    pub message: Option<String>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for reverting a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Revert a Gerrit change")]
pub struct RevertChangeParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Optional message for the revert commit")]
    pub message: Option<String>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for reverting a submission.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Revert a Gerrit submission")]
pub struct RevertSubmissionParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Optional message for the revert submission")]
    pub message: Option<String>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for posting a review comment on a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Post a review comment on a Gerrit change")]
pub struct PostReviewCommentParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Path to the file to comment on")]
    pub file_path: String,

    #[schemars(description = "Line number for the comment")]
    pub line_number: u64,

    #[schemars(description = "Comment message text")]
    pub message: String,

    #[serde(default = "default_true")]
    #[schemars(default, description = "Whether the comment is unresolved")]
    pub unresolved: Option<bool>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,

    #[serde(default)]
    #[schemars(default, description = "Vote labels as key-value pairs (e.g. 'Code-Review': 1)")]
    pub labels: Option<BTreeMap<String, i32>>,
}

/// Parameters for posting a draft comment on a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Post a draft comment on a Gerrit change")]
pub struct PostDraftCommentParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Path to the file to comment on")]
    pub file_path: String,

    #[schemars(description = "Line number for the comment")]
    pub line_number: u64,

    #[schemars(description = "Comment message text")]
    pub message: String,

    #[serde(default = "default_true")]
    #[schemars(default, description = "Whether the comment is unresolved")]
    pub unresolved: Option<bool>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,

    #[serde(default)]
    #[schemars(
        default,
        description = "Start line for a range comment"
    )]
    pub start_line: Option<u64>,

    #[serde(default)]
    #[schemars(
        default,
        description = "Start character offset for a range comment"
    )]
    pub start_character: Option<u64>,

    #[serde(default)]
    #[schemars(
        default,
        description = "End line for a range comment"
    )]
    pub end_line: Option<u64>,

    #[serde(default)]
    #[schemars(
        default,
        description = "End character offset for a range comment"
    )]
    pub end_character: Option<u64>,

    #[serde(default)]
    #[schemars(default, description = "Suggested code fix for the comment")]
    pub suggestion: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Draft comment ID to reply to")]
    pub in_reply_to: Option<String>,
}

/// Parameters for deleting a specific draft comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Delete a specific draft comment on a Gerrit change")]
pub struct DeleteDraftCommentParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Draft comment ID to delete")]
    pub draft_id: String,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for deleting all draft comments on a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Delete all draft comments on a Gerrit change")]
pub struct DeleteDraftCommentsParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for publishing draft comments and optionally submitting votes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Publish draft comments on a Gerrit change")]
pub struct PublishDraftsParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default)]
    #[schemars(default, description = "Optional message for the publication")]
    pub message: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Vote labels as key-value pairs")]
    pub labels: Option<BTreeMap<String, i32>>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

// ---------------------------------------------------------------------------
// Cherry-pick tools (2)
// ---------------------------------------------------------------------------

/// Parameters for cherry-picking a change to a destination branch.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Cherry-pick a Gerrit change to a destination branch")]
pub struct CherryPickChangeParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Destination branch for the cherry-pick")]
    pub destination: String,

    #[serde(default = "default_current_revision")]
    #[schemars(default, description = "Revision to cherry-pick (defaults to current)")]
    pub revision_id: Option<String>,

    #[serde(default)]
    #[schemars(default, description = "Message for the cherry-pick commit")]
    pub message: Option<String>,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to keep original reviewers on the cherry-pick")]
    pub keep_reviewers: Option<bool>,

    #[serde(default = "default_true")]
    #[schemars(default, description = "Whether to allow cherry-pick with conflicts")]
    pub allow_conflicts: Option<bool>,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to allow an empty cherry-pick")]
    pub allow_empty: Option<bool>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

/// Parameters for cherry-picking a chain of changes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Cherry-pick a chain of Gerrit changes to a destination branch")]
pub struct CherryPickChainParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[schemars(description = "Destination branch for the cherry-pick")]
    pub destination: String,

    #[serde(default = "default_current_revision")]
    #[schemars(default, description = "Revision to cherry-pick (defaults to current)")]
    pub revision_id: Option<String>,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to keep original reviewers on the cherry-pick")]
    pub keep_reviewers: Option<bool>,

    #[serde(default = "default_true")]
    #[schemars(default, description = "Whether to allow cherry-pick with conflicts")]
    pub allow_conflicts: Option<bool>,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to allow an empty cherry-pick")]
    pub allow_empty: Option<bool>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

// ---------------------------------------------------------------------------
// Submit tool (1)
// ---------------------------------------------------------------------------

/// Parameters for submitting a change for merge.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Submit a Gerrit change for merge")]
pub struct SubmitChangeParams {
    #[schemars(description = "Gerrit change ID (numeric or Change-Id hash)")]
    pub change_id: String,

    #[serde(default = "default_false")]
    #[schemars(default, description = "Whether to wait for the merge to complete")]
    pub wait_for_merge: Option<bool>,

    #[schemars(description = "Base URL of the Gerrit instance (overrides config)")]
    pub gerrit_base_url: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_changes_params_defaults() {
        let params: QueryChangesParams =
            serde_json::from_str(r#"{"query": "status:open"}"#).unwrap();
        assert_eq!(params.query, "status:open");
        assert_eq!(params.gerrit_base_url, None);
        assert_eq!(params.limit, None);
        assert_eq!(params.options, None);
    }

    #[test]
    fn cherry_pick_chain_params_defaults() {
        let params: CherryPickChainParams = serde_json::from_str(
            r#"{"change_id": "12345", "destination": "main", "gerrit_base_url": "https://gerrit.example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.change_id, "12345");
        assert_eq!(params.destination, "main");
        assert_eq!(params.revision_id, Some("current".to_string()));
        assert_eq!(params.keep_reviewers, Some(false));
        assert_eq!(params.allow_conflicts, Some(true));
        assert_eq!(params.allow_empty, Some(false));
    }

    #[test]
    fn submit_change_params_defaults() {
        let params: SubmitChangeParams = serde_json::from_str(
            r#"{"change_id": "12345", "gerrit_base_url": "https://gerrit.example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.change_id, "12345");
        assert_eq!(params.wait_for_merge, Some(false));
        assert_eq!(params.gerrit_base_url, "https://gerrit.example.com");
    }

    #[test]
    fn query_changes_by_date_params_defaults() {
        let params: QueryChangesByDateParams = serde_json::from_str(
            r#"{"start_date": "2026-01-01", "end_date": "2026-01-31"}"#,
        )
        .unwrap();
        assert_eq!(params.status, Some("merged".to_string()));
        assert_eq!(params.limit, None);
        assert_eq!(params.project, None);
    }

    #[test]
    fn suggest_reviewers_params_defaults() {
        let params: SuggestReviewersParams =
            serde_json::from_str(r#"{"change_id": "123", "query": "j"}"#).unwrap();
        assert_eq!(params.exclude_groups, Some(false));
        assert_eq!(params.limit, None);
        assert_eq!(params.reviewer_state, None);
    }

    #[test]
    fn add_reviewer_params_defaults() {
        let params: AddReviewerParams = serde_json::from_str(
            r#"{"change_id": "123", "reviewer": "user@example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.state, Some("REVIEWER".to_string()));
    }

    #[test]
    fn post_review_comment_params_defaults() {
        let params: PostReviewCommentParams = serde_json::from_str(
            r#"{"change_id": "123", "file_path": "src/main.rs", "line_number": 42, "message": "looks good", "gerrit_base_url": "https://g.example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.unresolved, Some(true));
        assert_eq!(params.labels, None);
    }

    #[test]
    fn post_draft_comment_params_defaults() {
        let params: PostDraftCommentParams = serde_json::from_str(
            r#"{"change_id": "123", "file_path": "src/main.rs", "line_number": 42, "message": "draft", "gerrit_base_url": "https://g.example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.unresolved, Some(true));
        assert_eq!(params.start_line, None);
        assert_eq!(params.suggestion, None);
    }

    #[test]
    fn cherry_pick_change_params_defaults() {
        let params: CherryPickChangeParams = serde_json::from_str(
            r#"{"change_id": "12345", "destination": "release", "gerrit_base_url": "https://gerrit.example.com"}"#,
        )
        .unwrap();
        assert_eq!(params.revision_id, Some("current".to_string()));
        assert_eq!(params.keep_reviewers, Some(false));
        assert_eq!(params.allow_conflicts, Some(true));
        assert_eq!(params.allow_empty, Some(false));
        assert_eq!(params.message, None);
    }

    #[test]
    fn no_params_deserializes_empty_json() {
        let _params: NoParams = serde_json::from_str("{}").unwrap();
    }
}
