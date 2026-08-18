// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

pub mod changes;
pub mod comments;
pub mod reviews;
pub mod tools;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gerrit_core::domain::*;
use gerrit_core::infrastructure::client::{GerritClient, GerritClientConfig};
use regex_lite::Regex;
use rmcp::{
    handler::server::{ServerHandler, tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::health::metrics;
use crate::mcp::tools::*;

pub(crate) const GERRIT_OPTION_CURRENT_REVISION: &str = "CURRENT_REVISION";
pub(crate) const GERRIT_OPTION_CURRENT_COMMIT: &str = "CURRENT_COMMIT";
pub(crate) const GERRIT_OPTION_DETAILED_LABELS: &str = "DETAILED_LABELS";
pub(crate) const REVIEWER_STATE_REVIEWER: &str = "REVIEWER";
pub(crate) const DEFAULT_REVISION: &str = "current";
pub(crate) const DEFAULT_STATUS_MERGED: &str = "merged";

pub(crate) fn extract_bugs(commit_message: &str) -> Vec<String> {
    let mut bugs: Vec<String> = Vec::new();

    let prefix_re = Regex::new(r"(?im)^(?:Bug|Fixes|Closes):\s*(.+)").unwrap();
    for cap in prefix_re.captures_iter(commit_message) {
        if let Some(m) = cap.get(1) {
            for part in m.as_str().split(',') {
                let trimmed = part.trim();
                if let Some(num) = trimmed.split(|c: char| !c.is_ascii_digit()).next()
                    && !num.is_empty()
                {
                    bugs.push(num.to_string());
                }
            }
        }
    }

    let inline_re = Regex::new(r"b/(\d+)").unwrap();
    for cap in inline_re.captures_iter(commit_message) {
        if let Some(m) = cap.get(1) {
            bugs.push(m.as_str().to_string());
        }
    }

    bugs.sort_unstable();
    bugs.dedup();
    bugs
}

pub(crate) fn sort_by_date(changes: &mut [Change]) {
    changes.sort_by(|a, b| b.updated.cmp(&a.updated));
}

pub(crate) fn format_change_line(change: &Change) -> String {
    let wip = if change.work_in_progress {
        "[WIP] "
    } else {
        ""
    };
    let topic = change
        .topic
        .as_ref()
        .map(|t| format!(" [topic:{}]", t))
        .unwrap_or_default();
    format!(
        "{}_{}: {}{}{}",
        change._number, change.updated, wip, change.subject, topic
    )
}

pub(crate) fn format_changes_output(changes: &[Change]) -> String {
    changes
        .iter()
        .map(format_change_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct GerritServer<R: GerritRepository + Send + Sync + 'static> {
    pub repo: Arc<R>,
    tool_router: ToolRouter<Self>,
    client_config: Option<GerritClientConfig>,
    client_cache: Arc<Mutex<HashMap<String, Arc<GerritClient>>>>,
    read_only: bool,
}

impl<R: GerritRepository + Send + Sync + 'static> Clone for GerritServer<R> {
    fn clone(&self) -> Self {
        Self {
            repo: self.repo.clone(),
            tool_router: self.tool_router.clone(),
            client_config: self.client_config.clone(),
            client_cache: self.client_cache.clone(),
            read_only: self.read_only,
        }
    }
}

impl<R: GerritRepository + Send + Sync + 'static> GerritServer<R> {
    pub fn new(repo: R) -> Self {
        Self {
            repo: Arc::new(repo),
            tool_router: ToolRouter::new(),
            client_config: None,
            client_cache: Arc::new(Mutex::new(HashMap::new())),
            read_only: false,
        }
    }

    pub fn with_client_factory(mut self, config: GerritClientConfig) -> Self {
        self.client_config = Some(config);
        self
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    fn check_not_readonly(&self, action: &str) -> Option<CallToolResult> {
        if self.read_only {
            Some(self.error(format!("Cannot {action} in read-only mode.")))
        } else {
            None
        }
    }

    fn resolve_client(&self, override_url: Option<&str>) -> Result<Arc<GerritClient>, String> {
        let url = match override_url {
            Some(u) => u.to_string(),
            None => return Err("gerrit_base_url not provided".to_string()),
        };
        let normalized = url.trim_end_matches('/').to_string();

        let mut cache = self.client_cache.lock().unwrap();
        if let Some(existing) = cache.get(&normalized) {
            return Ok(existing.clone());
        }

        let config = self.client_config.as_ref().ok_or_else(|| {
            "client factory not configured (no GerritClientConfig available)".to_string()
        })?;
        let mut cfg = config.clone();
        cfg.base_url = normalized.clone();

        let client = GerritClient::new(cfg)
            .map_err(|e| format!("failed to create client for {normalized}: {e}"))?;
        let client = Arc::new(client);
        cache.insert(normalized, client.clone());
        Ok(client)
    }

    #[allow(dead_code)] // used by write-tool handlers in follow-up tasks
    fn resolve_repo(
        &self,
        override_url: Option<&str>,
    ) -> Result<Arc<dyn GerritRepository>, CallToolResult> {
        match override_url {
            Some(url) => match self.resolve_client(Some(url)) {
                Ok(client) => Ok(client as Arc<dyn GerritRepository>),
                Err(e) => Err(self.error(format!("Failed to resolve client for {url}: {e}"))),
            },
            None => Ok(self.repo.clone() as Arc<dyn GerritRepository>),
        }
    }

    fn text(&self, text: String) -> CallToolResult {
        metrics().record_tool_call();
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    fn error(&self, msg: String) -> CallToolResult {
        metrics().record_tool_error();
        CallToolResult::error(vec![ContentBlock::text(msg)])
    }

    fn merge_options(base: &[&str], extra: &[String]) -> Vec<String> {
        let mut opts: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        for e in extra {
            if !opts.contains(e) {
                opts.push(e.clone());
            }
        }
        opts
    }
}

#[tool_router]
impl<R: GerritRepository + Send + Sync + 'static> GerritServer<R> {
    #[tool(
        name = "query_changes",
        description = "Query Gerrit changes using a search query string"
    )]
    pub async fn query_changes(
        &self,
        Parameters(params): Parameters<QueryChangesParams>,
    ) -> CallToolResult {
        changes::query_changes(self, params).await
    }

    #[tool(
        name = "query_changes_by_date_and_filters",
        description = "Query Gerrit changes within a date range with optional filters"
    )]
    pub async fn query_changes_by_date_and_filters(
        &self,
        Parameters(params): Parameters<QueryChangesByDateParams>,
    ) -> CallToolResult {
        changes::query_changes_by_date_and_filters(self, params).await
    }

    #[tool(
        name = "get_change_details",
        description = "Get detailed information about a Gerrit change"
    )]
    pub async fn get_change_details(
        &self,
        Parameters(params): Parameters<GetChangeDetailsParams>,
    ) -> CallToolResult {
        changes::get_change_details(self, params).await
    }

    #[tool(
        name = "get_commit_message",
        description = "Get the commit message for a Gerrit change"
    )]
    pub async fn get_commit_message(
        &self,
        Parameters(params): Parameters<GetCommitMessageParams>,
    ) -> CallToolResult {
        changes::get_commit_message(self, params).await
    }

    #[tool(
        name = "get_most_recent_cl",
        description = "Get the most recent change from a Gerrit user"
    )]
    pub async fn get_most_recent_cl(
        &self,
        Parameters(params): Parameters<GetMostRecentClParams>,
    ) -> CallToolResult {
        changes::get_most_recent_cl(self, params).await
    }

    #[tool(
        name = "get_bugs_from_cl",
        description = "Extract bug references from a Gerrit change"
    )]
    pub async fn get_bugs_from_cl(
        &self,
        Parameters(params): Parameters<GetBugsFromClParams>,
    ) -> CallToolResult {
        changes::get_bugs_from_cl(self, params).await
    }

    #[tool(name = "create_change", description = "Create a new change in Gerrit")]
    pub async fn create_change(
        &self,
        Parameters(params): Parameters<CreateChangeParams>,
    ) -> CallToolResult {
        changes::create_change(self, params).await
    }

    #[tool(
        name = "changes_submitted_together",
        description = "List changes submitted together with this one"
    )]
    pub async fn changes_submitted_together(
        &self,
        Parameters(params): Parameters<ChangesSubmittedTogetherParams>,
    ) -> CallToolResult {
        changes::changes_submitted_together(self, params).await
    }

    #[tool(
        name = "set_ready_for_review",
        description = "Mark a Gerrit change as ready for review"
    )]
    pub async fn set_ready_for_review(
        &self,
        Parameters(params): Parameters<SetReadyParams>,
    ) -> CallToolResult {
        changes::set_ready_for_review(self, params).await
    }

    #[tool(
        name = "set_work_in_progress",
        description = "Mark a Gerrit change as work-in-progress"
    )]
    pub async fn set_work_in_progress(
        &self,
        Parameters(params): Parameters<SetWipParams>,
    ) -> CallToolResult {
        changes::set_work_in_progress(self, params).await
    }

    #[tool(name = "set_topic", description = "Set the topic for a Gerrit change")]
    pub async fn set_topic(
        &self,
        Parameters(params): Parameters<SetTopicParams>,
    ) -> CallToolResult {
        changes::set_topic(self, params).await
    }

    #[tool(name = "abandon_change", description = "Abandon a Gerrit change")]
    pub async fn abandon_change(
        &self,
        Parameters(params): Parameters<AbandonChangeParams>,
    ) -> CallToolResult {
        changes::abandon_change(self, params).await
    }

    #[tool(name = "revert_change", description = "Revert a Gerrit change")]
    pub async fn revert_change(
        &self,
        Parameters(params): Parameters<RevertChangeParams>,
    ) -> CallToolResult {
        changes::revert_change(self, params).await
    }

    #[tool(name = "revert_submission", description = "Revert a Gerrit submission")]
    pub async fn revert_submission(
        &self,
        Parameters(params): Parameters<RevertSubmissionParams>,
    ) -> CallToolResult {
        changes::revert_submission(self, params).await
    }

    #[tool(
        name = "submit_change",
        description = "Submit a Gerrit change for merge"
    )]
    pub async fn submit_change(
        &self,
        Parameters(params): Parameters<SubmitChangeParams>,
    ) -> CallToolResult {
        changes::submit_change(self, params).await
    }

    #[tool(
        name = "list_change_comments",
        description = "List published comments on a Gerrit change"
    )]
    pub async fn list_change_comments(
        &self,
        Parameters(params): Parameters<ListChangeCommentsParams>,
    ) -> CallToolResult {
        comments::list_change_comments(self, params).await
    }

    #[tool(
        name = "list_draft_comments",
        description = "List draft comments on a Gerrit change"
    )]
    pub async fn list_draft_comments(
        &self,
        Parameters(params): Parameters<ListDraftCommentsParams>,
    ) -> CallToolResult {
        comments::list_draft_comments(self, params).await
    }

    #[tool(
        name = "post_review_comment",
        description = "Post a review comment on a Gerrit change"
    )]
    pub async fn post_review_comment(
        &self,
        Parameters(params): Parameters<PostReviewCommentParams>,
    ) -> CallToolResult {
        comments::post_review_comment(self, params).await
    }

    #[tool(
        name = "set_labels",
        description = "Set one or more label votes on a Gerrit change"
    )]
    pub async fn set_labels(
        &self,
        Parameters(params): Parameters<SetLabelsParams>,
    ) -> CallToolResult {
        changes::set_labels(self, params).await
    }

    #[tool(
        name = "post_draft_comment",
        description = "Post a draft comment on a Gerrit change"
    )]
    pub async fn post_draft_comment(
        &self,
        Parameters(params): Parameters<PostDraftCommentParams>,
    ) -> CallToolResult {
        comments::post_draft_comment(self, params).await
    }

    #[tool(
        name = "delete_draft_comment",
        description = "Delete a specific draft comment on a Gerrit change"
    )]
    pub async fn delete_draft_comment(
        &self,
        Parameters(params): Parameters<DeleteDraftCommentParams>,
    ) -> CallToolResult {
        comments::delete_draft_comment(self, params).await
    }

    #[tool(
        name = "delete_draft_comments",
        description = "Delete all draft comments on a Gerrit change"
    )]
    pub async fn delete_draft_comments(
        &self,
        Parameters(params): Parameters<DeleteDraftCommentsParams>,
    ) -> CallToolResult {
        comments::delete_draft_comments(self, params).await
    }

    #[tool(
        name = "publish_drafts",
        description = "Publish draft comments on a Gerrit change"
    )]
    pub async fn publish_drafts(
        &self,
        Parameters(params): Parameters<PublishDraftsParams>,
    ) -> CallToolResult {
        comments::publish_drafts(self, params).await
    }

    #[tool(
        name = "list_change_files",
        description = "List files modified in a Gerrit change"
    )]
    pub async fn list_change_files(
        &self,
        Parameters(params): Parameters<ListChangeFilesParams>,
    ) -> CallToolResult {
        reviews::list_change_files(self, params).await
    }

    #[tool(
        name = "get_file_diff",
        description = "Get the diff for a file in a Gerrit change"
    )]
    pub async fn get_file_diff(
        &self,
        Parameters(params): Parameters<GetFileDiffParams>,
    ) -> CallToolResult {
        reviews::get_file_diff(self, params).await
    }

    #[tool(
        name = "suggest_reviewers",
        description = "Suggest reviewers for a Gerrit change"
    )]
    pub async fn suggest_reviewers(
        &self,
        Parameters(params): Parameters<SuggestReviewersParams>,
    ) -> CallToolResult {
        reviews::suggest_reviewers(self, params).await
    }

    #[tool(
        name = "add_reviewer",
        description = "Add a reviewer to a Gerrit change"
    )]
    pub async fn add_reviewer(
        &self,
        Parameters(params): Parameters<AddReviewerParams>,
    ) -> CallToolResult {
        reviews::add_reviewer(self, params).await
    }

    #[tool(
        name = "cherry_pick_change",
        description = "Cherry-pick a Gerrit change to a destination branch"
    )]
    pub async fn cherry_pick_change(
        &self,
        Parameters(params): Parameters<CherryPickChangeParams>,
    ) -> CallToolResult {
        reviews::cherry_pick_change(self, params).await
    }

    #[tool(
        name = "cherry_pick_chain",
        description = "Cherry-pick a chain of Gerrit changes to a destination branch"
    )]
    pub async fn cherry_pick_chain(
        &self,
        Parameters(params): Parameters<CherryPickChainParams>,
    ) -> CallToolResult {
        reviews::cherry_pick_chain(self, params).await
    }
}

#[tool_handler]
impl<R: GerritRepository + Send + Sync + 'static> ServerHandler for GerritServer<R> {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.instructions = Some(
            "Gerrit MCP server for code review. Provides tools for querying changes, \
             reviewing code, managing reviews, cherry-picking, and submitting changes."
                .into(),
        );
        info
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28, ProtocolVersion::V_2025_11_25])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gerrit_core::domain::MockGerritRepository;
    use std::collections::BTreeMap;

    fn extract_text(result: CallToolResult) -> String {
        if let Some(text) = result.content.iter().find_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        }) {
            return text;
        }
        String::new()
    }

    #[tokio::test]
    pub async fn test_query_changes_empty_result() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![]));
        let server = GerritServer::new(mock);

        let params = QueryChangesParams {
            query: "status:open".to_string(),
            gerrit_base_url: None,
            limit: None,
            options: None,
        };
        let result = server.query_changes(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("No changes found for query: status:open"));
    }

    #[tokio::test]
    pub async fn test_query_changes_with_results() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![MockGerritRepository::make_change(
            12345,
            "Test Subject",
        )]));
        let server = GerritServer::new(mock);

        let params = QueryChangesParams {
            query: "status:open".to_string(),
            gerrit_base_url: None,
            limit: None,
            options: None,
        };
        let result = server.query_changes(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("12345_"));
        assert!(text.contains("Test Subject"));
    }

    #[tokio::test]
    pub async fn test_cherry_pick_change_success() {
        let mock = MockGerritRepository::default();
        mock.push_cherry_pick_result(Ok(CherryPickResult {
            id: "new~999".into(),
            _number: 999,
            subject: "Cherry-picked".into(),
        }));
        let server = GerritServer::new(mock);

        let params = CherryPickChangeParams {
            change_id: "12345".to_string(),
            destination: "main".to_string(),
            revision_id: None,
            message: None,
            keep_reviewers: None,
            allow_conflicts: None,
            allow_empty: None,
            gerrit_base_url: None,
        };
        let result = server.cherry_pick_change(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("Successfully cherry-picked"));
        assert!(text.contains("999"));
    }

    #[tokio::test]
    pub async fn test_cherry_pick_chain_success() {
        let mock = MockGerritRepository::default();

        mock.push_get_related_result(Ok(vec![
            RelatedChange {
                _change_number: 2,
                _revision_number: 1,
            },
            RelatedChange {
                _change_number: 1,
                _revision_number: 1,
            },
        ]));

        mock.push_cherry_pick_result(Ok(CherryPickResult {
            id: "new~100".into(),
            _number: 100,
            subject: "Cp1".into(),
        }));
        mock.push_get_change_detail_result(Ok(ChangeDetail {
            id: "new~100".into(),
            _number: 100,
            subject: "Cp1".into(),
            status: "NEW".into(),
            project: "p".into(),
            branch: "b".into(),
            owner: AccountInfo {
                _account_id: 1,
                name: None,
                email: None,
            },
            updated: "now".into(),
            current_revision: Some("rev1".into()),
            current_revision_number: Some(1),
            revisions: {
                let mut m = BTreeMap::new();
                m.insert(
                    "rev1".into(),
                    RevisionInfo {
                        _number: 1,
                        commit: Some(CommitWithMessage {
                            message: "abc123".into(),
                        }),
                    },
                );
                m
            },
            labels: BTreeMap::new(),
            reviewers: None,
            messages: vec![],
            topic: None,
        }));

        mock.push_cherry_pick_result(Ok(CherryPickResult {
            id: "new~101".into(),
            _number: 101,
            subject: "Cp2".into(),
        }));
        mock.push_get_change_detail_result(Ok(ChangeDetail {
            id: "new~101".into(),
            _number: 101,
            subject: "Cp2".into(),
            status: "NEW".into(),
            project: "p".into(),
            branch: "b".into(),
            owner: AccountInfo {
                _account_id: 2,
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
            topic: None,
        }));

        let server = GerritServer::new(mock);

        let params = CherryPickChainParams {
            change_id: "2".to_string(),
            destination: "main".to_string(),
            revision_id: None,
            keep_reviewers: None,
            allow_conflicts: None,
            allow_empty: None,
            gerrit_base_url: None,
        };
        let result = server.cherry_pick_chain(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("Successfully cherry-picked chain of 2"));
    }

    #[tokio::test]
    pub async fn test_submit_change_success() {
        let mock = MockGerritRepository::default();
        mock.push_submit_change_result(Ok(SubmitResult {
            id: "test~42".into(),
            _number: 42,
            subject: "Merged change".into(),
            status: "MERGED".into(),
        }));
        let server = GerritServer::new(mock);

        let params = SubmitChangeParams {
            change_id: "42".to_string(),
            wait_for_merge: None,
            gerrit_base_url: None,
        };
        let result = server.submit_change(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("Successfully submitted"));
        assert!(text.contains("MERGED"));
    }

    #[test]
    fn test_extract_bugs_prefixes() {
        let msg = "Bug: 12345, 67890\nFixes: 11111\nSome text\nCloses: 22222";
        let bugs = extract_bugs(msg);
        assert!(bugs.contains(&"11111".to_string()));
        assert!(bugs.contains(&"12345".to_string()));
        assert!(bugs.contains(&"22222".to_string()));
        assert!(bugs.contains(&"67890".to_string()));
    }

    #[test]
    fn test_extract_bugs_inline() {
        let msg = "Fixed b/12345 and also b/99999";
        let bugs = extract_bugs(msg);
        assert!(bugs.contains(&"12345".to_string()));
        assert!(bugs.contains(&"99999".to_string()));
    }

    #[test]
    fn test_extract_bugs_dedup() {
        let msg = "Bug: 12345\nFixes: 12345\nb/12345";
        let bugs = extract_bugs(msg);
        assert_eq!(bugs, vec!["12345"]);
    }

    #[test]
    fn test_extract_bugs_empty() {
        let bugs = extract_bugs("No bugs here.");
        assert!(bugs.is_empty());
    }

    #[test]
    fn test_sort_by_date() {
        let mut changes = vec![
            Change {
                id: "a".into(),
                _number: 1,
                subject: "old".into(),
                status: "NEW".into(),
                project: "p".into(),
                branch: "b".into(),
                owner: AccountInfo {
                    _account_id: 1,
                    name: None,
                    email: None,
                },
                updated: "2020-01-01 00:00:00".into(),
                work_in_progress: false,
                topic: None,
                reviewers: None,
            },
            Change {
                id: "b".into(),
                _number: 2,
                subject: "new".into(),
                status: "NEW".into(),
                project: "p".into(),
                branch: "b".into(),
                owner: AccountInfo {
                    _account_id: 2,
                    name: None,
                    email: None,
                },
                updated: "2025-06-15 12:00:00".into(),
                work_in_progress: false,
                topic: None,
                reviewers: None,
            },
        ];
        sort_by_date(&mut changes);
        assert_eq!(changes[0]._number, 2);
        assert_eq!(changes[1]._number, 1);
    }

    #[test]
    fn test_merge_options_dedup() {
        let base = &[GERRIT_OPTION_CURRENT_REVISION, GERRIT_OPTION_CURRENT_COMMIT];
        let extra = vec![
            GERRIT_OPTION_CURRENT_REVISION.to_string(),
            GERRIT_OPTION_DETAILED_LABELS.to_string(),
        ];
        let opts = GerritServer::<MockGerritRepository>::merge_options(base, &extra);
        assert_eq!(opts.len(), 3);
        assert!(opts.contains(&GERRIT_OPTION_CURRENT_REVISION.to_string()));
        assert!(opts.contains(&GERRIT_OPTION_CURRENT_COMMIT.to_string()));
        assert!(opts.contains(&GERRIT_OPTION_DETAILED_LABELS.to_string()));
    }

    #[tokio::test]
    async fn test_read_only_mode_blocks_write() {
        let mock = MockGerritRepository::default();
        let server = GerritServer::new(mock).with_read_only(true);

        let params = CreateChangeParams {
            project: "test".into(),
            branch: "main".into(),
            subject: "Test".into(),
            topic: None,
            status: None,
            gerrit_base_url: None,
        };
        let result = server.create_change(Parameters(params)).await;
        assert!(result.is_error.unwrap_or(false));
        let text = extract_text(result);
        assert!(
            text.contains("read-only"),
            "expected read-only error, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_read_allowed_in_read_only_mode() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![MockGerritRepository::make_change(
            12345,
            "Test Subject",
        )]));
        let server = GerritServer::new(mock).with_read_only(true);

        let params = QueryChangesParams {
            query: "status:open".to_string(),
            gerrit_base_url: None,
            limit: None,
            options: None,
        };
        let result = server.query_changes(Parameters(params)).await;
        assert!(!result.is_error.unwrap_or(true));
    }

    #[tokio::test]
    async fn test_read_only_mode_blocks_abandon() {
        let mock = MockGerritRepository::default();
        let server = GerritServer::new(mock).with_read_only(true);

        let params = AbandonChangeParams {
            change_id: "12345".into(),
            message: None,
            gerrit_base_url: None,
        };
        let result = server.abandon_change(Parameters(params)).await;
        assert!(result.is_error.unwrap_or(false));
        let text = extract_text(result);
        assert!(
            text.contains("read-only"),
            "expected read-only error for abandon, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_set_labels_success() {
        let mock = MockGerritRepository::default();
        mock.push_set_labels_result(Ok(()));
        let server = GerritServer::new(mock);
        let params = SetLabelsParams {
            change_id: "123".into(),
            labels: BTreeMap::from([("READY-FOR-CI".into(), 1)]),
            message: Some("Trigger".into()),
            gerrit_base_url: None,
        };
        let result = server.set_labels(Parameters(params)).await;
        let text = extract_text(result);
        assert!(text.contains("Labels set on change 123: READY-FOR-CI=1"));
    }

    #[tokio::test]
    async fn test_set_labels_read_only_blocks() {
        let mock = MockGerritRepository::default();
        let server = GerritServer::new(mock).with_read_only(true);
        let params = SetLabelsParams {
            change_id: "123".into(),
            labels: BTreeMap::from([("READY-FOR-CI".into(), 1)]),
            message: None,
            gerrit_base_url: Some("https://g.example.com".into()),
        };
        let result = server.set_labels(Parameters(params)).await;
        assert!(result.is_error.unwrap_or(false));
        let text = extract_text(result);
        assert!(text.contains("read-only"));
    }

    #[tokio::test]
    async fn test_set_labels_error() {
        let mock = MockGerritRepository::default();
        mock.push_set_labels_result(Err(DomainError::HttpStatus {
            status: 403,
            body: "forbidden".into(),
        }));
        let server = GerritServer::new(mock);
        let params = SetLabelsParams {
            change_id: "123".into(),
            labels: BTreeMap::from([("READY-FOR-CI".into(), 1)]),
            message: None,
            gerrit_base_url: None,
        };
        let result = server.set_labels(Parameters(params)).await;
        assert!(result.is_error.unwrap_or(false));
        let text = extract_text(result);
        assert!(text.contains("Failed to set labels"));
    }

    #[tokio::test]
    async fn test_resolve_repo_none_returns_repo() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![MockGerritRepository::make_change(
            1,
            "via helper",
        )]));
        let server = GerritServer::new(mock);
        let repo = server
            .resolve_repo(None)
            .expect("resolve_repo(None) should succeed");
        let changes = repo.query_changes("q", None, &[]).await.unwrap();
        assert_eq!(changes[0]._number, 1);
    }

    #[tokio::test]
    async fn test_resolve_repo_some_without_factory_errors() {
        let mock = MockGerritRepository::default();
        let server = GerritServer::new(mock);
        let err = server
            .resolve_repo(Some("https://override.example.com"))
            .err()
            .expect("resolve_repo(Some) without factory should fail");
        let text = extract_text(err);
        assert!(
            text.contains("client factory not configured"),
            "got: {text}"
        );
    }

    #[tokio::test]
    async fn test_resolve_repo_some_with_bad_client_config_errors() {
        use gerrit_core::infrastructure::auth::AuthMode;
        use gerrit_core::infrastructure::client::GerritClientConfig;
        use gerrit_core::infrastructure::tls::TlsConfig;
        use std::time::Duration;
        let mock = MockGerritRepository::default();
        let cfg = GerritClientConfig {
            base_url: "https://override.example.com".into(),
            auth: AuthMode::Bearer("t".into()),
            timeout: Duration::from_secs(1),
            tls: TlsConfig {
                verify_ssl: true,
                ca_cert: Some("/nonexistent/ca-cert.pem".into()),
                ca_cert_dir: None,
            },
            disable_url_normalization: true,
        };
        let server = GerritServer::new(mock).with_client_factory(cfg);
        let err = server
            .resolve_repo(Some("https://override.example.com"))
            .err()
            .expect("resolve_repo(Some) with factory on bad host should fail");
        let text = extract_text(err);
        assert!(text.contains("Failed to resolve client"), "got: {text}");
    }
}
