// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

pub mod tools;

use std::collections::BTreeMap;
use std::sync::Arc;

use gerrit_core::domain::*;
use regex_lite::Regex;
use rmcp::{
    handler::server::{ServerHandler, tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::mcp::tools::*;

pub(crate) const GERRIT_OPTION_CURRENT_REVISION: &str = "CURRENT_REVISION";
pub(crate) const GERRIT_OPTION_CURRENT_COMMIT: &str = "CURRENT_COMMIT";
pub(crate) const GERRIT_OPTION_DETAILED_LABELS: &str = "DETAILED_LABELS";
pub(crate) const REVIEWER_STATE_REVIEWER: &str = "REVIEWER";
pub(crate) const REVIEWER_STATE_CC: &str = "CC";
pub(crate) const DEFAULT_REVISION: &str = "current";
pub(crate) const DEFAULT_STATUS_MERGED: &str = "merged";

fn extract_bugs(commit_message: &str) -> Vec<String> {
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

fn sort_by_date(changes: &mut [Change]) {
    changes.sort_by(|a, b| b.updated.cmp(&a.updated));
}

pub struct GerritServer<R: GerritRepository + Send + Sync + 'static> {
    pub repo: Arc<R>,
    tool_router: ToolRouter<Self>,
}

impl<R: GerritRepository + Send + Sync + 'static> Clone for GerritServer<R> {
    fn clone(&self) -> Self {
        Self {
            repo: self.repo.clone(),
            tool_router: self.tool_router.clone(),
        }
    }
}

impl<R: GerritRepository + Send + Sync + 'static> GerritServer<R> {
    pub fn new(repo: R) -> Self {
        Self {
            repo: Arc::new(repo),
            tool_router: ToolRouter::new(),
        }
    }

    fn text(&self, text: String) -> CallToolResult {
        CallToolResult::success(vec![ContentBlock::text(text)])
    }

    fn error(&self, msg: String) -> CallToolResult {
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
        let opts = params.options.unwrap_or_default();
        match self
            .repo
            .query_changes(&params.query, params.limit, &opts)
            .await
        {
            Ok(mut changes) => {
                if changes.is_empty() {
                    return self.text(format!("No changes found for query: {}", params.query));
                }
                sort_by_date(&mut changes);
                let mut lines = Vec::new();
                for c in &changes {
                    let wip = if c.work_in_progress { "[WIP] " } else { "" };
                    lines.push(format!("{}_{}: {}{}", c._number, c.updated, wip, c.subject));
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to query changes: {e}")),
        }
    }

    #[tool(
        name = "query_changes_by_date_and_filters",
        description = "Query Gerrit changes within a date range with optional filters"
    )]
    pub async fn query_changes_by_date_and_filters(
        &self,
        Parameters(params): Parameters<QueryChangesByDateParams>,
    ) -> CallToolResult {
        let end_date = match chrono::NaiveDate::parse_from_str(&params.end_date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => return self.error(format!("Invalid end_date format: {e}")),
        };
        let end_plus_one = end_date
            .succ_opt()
            .unwrap_or(end_date)
            .format("%Y-%m-%d")
            .to_string();

        let status = params.status.as_deref().unwrap_or(DEFAULT_STATUS_MERGED);
        let mut query = format!(
            "status:{} after:{} before:{}",
            status, params.start_date, end_plus_one
        );

        if let Some(ref project) = params.project {
            query.push_str(&format!(" project:{}", project));
        }
        if let Some(ref msg) = params.message_substring {
            query.push_str(&format!(" message:{}", msg));
        }

        let opts = Vec::new();
        match self.repo.query_changes(&query, params.limit, &opts).await {
            Ok(mut changes) => {
                if changes.is_empty() {
                    return self.text(format!("No changes found for query: {}", query));
                }
                sort_by_date(&mut changes);
                let mut lines = Vec::new();
                for c in &changes {
                    let wip = if c.work_in_progress { "[WIP] " } else { "" };
                    lines.push(format!("{}_{}: {}{}", c._number, c.updated, wip, c.subject));
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to query changes by date: {e}")),
        }
    }

    #[tool(
        name = "get_change_details",
        description = "Get detailed information about a Gerrit change"
    )]
    pub async fn get_change_details(
        &self,
        Parameters(params): Parameters<GetChangeDetailsParams>,
    ) -> CallToolResult {
        let base = &[
            GERRIT_OPTION_CURRENT_REVISION,
            GERRIT_OPTION_CURRENT_COMMIT,
            GERRIT_OPTION_DETAILED_LABELS,
        ];
        let extra = params.options.unwrap_or_default();
        let opts = Self::merge_options(base, &extra);

        match self.repo.get_change_detail(&params.change_id, &opts).await {
            Ok(detail) => {
                let mut lines = Vec::new();
                lines.push(format!("Subject: {}", detail.subject));
                lines.push(format!(
                    "Owner: {} <{}>",
                    detail.owner.name.as_deref().unwrap_or("unknown"),
                    detail.owner.email.as_deref().unwrap_or("no email")
                ));
                lines.push(format!("Status: {}", detail.status));

                let commit_msg = detail
                    .revisions
                    .values()
                    .find_map(|r| r.commit.as_ref().map(|c| c.message.clone()))
                    .unwrap_or_default();
                let bugs = extract_bugs(&commit_msg);
                if !bugs.is_empty() {
                    lines.push(format!("Bugs: {}", bugs.join(", ")));
                }

                if let Some(ref reviewers) = detail.reviewers
                    && let Some(reviewer_list) = reviewers.get(REVIEWER_STATE_REVIEWER)
                    && !reviewer_list.is_empty()
                {
                    for r in reviewer_list {
                        if let Some(ref email) = r.email {
                            lines.push(format!("Reviewer: {}", email));
                        }
                    }
                }

                for (label_name, label_info) in &detail.labels {
                    for vote in &label_info.all {
                        if let Some(val) = vote.value {
                            lines.push(format!(
                                "Vote {}: {} (by account {})",
                                label_name, val, vote._account_id
                            ));
                        }
                    }
                }

                let recent: Vec<&Message> = detail.messages.iter().rev().take(3).collect();
                if !recent.is_empty() {
                    lines.push("Recent messages:".to_string());
                    for msg in recent.iter().rev() {
                        let author = msg
                            .author
                            .as_ref()
                            .and_then(|a| a.email.clone())
                            .unwrap_or_else(|| "unknown".to_string());
                        lines.push(format!("  [{}] {}: {}", msg.date, author, msg.message));
                    }
                }

                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to get change details: {e}")),
        }
    }

    #[tool(
        name = "get_commit_message",
        description = "Get the commit message for a Gerrit change"
    )]
    pub async fn get_commit_message(
        &self,
        Parameters(params): Parameters<GetCommitMessageParams>,
    ) -> CallToolResult {
        match self.repo.get_commit_message(&params.change_id).await {
            Ok(msg) => {
                let mut lines = Vec::new();
                lines.push(format!("Subject: {}", msg.subject));
                lines.push(msg.full_message.clone());
                if !msg.footers.is_empty() {
                    lines.push("Footers:".to_string());
                    for (k, v) in &msg.footers {
                        lines.push(format!("  {}: {}", k, v));
                    }
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to get commit message: {e}")),
        }
    }

    #[tool(
        name = "list_change_files",
        description = "List files modified in a Gerrit change"
    )]
    pub async fn list_change_files(
        &self,
        Parameters(params): Parameters<ListChangeFilesParams>,
    ) -> CallToolResult {
        match self.repo.list_files(&params.change_id).await {
            Ok(files) => {
                let mut lines = Vec::new();
                for (path, info) in &files {
                    if path == "/COMMIT_MSG" {
                        continue;
                    }
                    let status_char = match info.status.as_deref() {
                        Some("A") => 'A',
                        Some("D") => 'D',
                        Some("R") => 'R',
                        Some("W") => 'W',
                        Some("M") => 'M',
                        _ => '?',
                    };
                    lines.push(format!(
                        "[{}] {} (+{}, -{})",
                        status_char, path, info.lines_inserted, info.lines_deleted
                    ));
                }
                if lines.is_empty() {
                    self.text("No files found.".to_string())
                } else {
                    self.text(lines.join("\n"))
                }
            }
            Err(e) => self.error(format!("Failed to list change files: {e}")),
        }
    }

    #[tool(
        name = "get_file_diff",
        description = "Get the diff for a file in a Gerrit change"
    )]
    pub async fn get_file_diff(
        &self,
        Parameters(params): Parameters<GetFileDiffParams>,
    ) -> CallToolResult {
        match self
            .repo
            .get_diff(&params.change_id, &params.file_path)
            .await
        {
            Ok(text) => self.text(text),
            Err(e) => self.error(format!("Failed to get file diff: {e}")),
        }
    }

    #[tool(
        name = "list_change_comments",
        description = "List published comments on a Gerrit change"
    )]
    pub async fn list_change_comments(
        &self,
        Parameters(params): Parameters<ListChangeCommentsParams>,
    ) -> CallToolResult {
        match self.repo.list_comments(&params.change_id).await {
            Ok(comments) => {
                if comments.is_empty() {
                    return self.text("No comments.".to_string());
                }
                let mut lines = Vec::new();
                for (file, file_comments) in &comments {
                    lines.push(format!("File: {}", file));
                    for c in file_comments {
                        let author = c
                            .author
                            .as_ref()
                            .and_then(|a| a.email.clone())
                            .unwrap_or_else(|| "unknown".to_string());
                        let resolved = if c.unresolved == Some(true) {
                            "[unresolved]"
                        } else {
                            "[resolved]"
                        };
                        let line_str = c
                            .line
                            .map(|l| format!("L{}", l))
                            .unwrap_or_else(|| "N/A".to_string());
                        lines.push(format!(
                            "  {} {} [{}] {}: {}",
                            line_str, resolved, c.id, author, c.message
                        ));
                    }
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to list comments: {e}")),
        }
    }

    #[tool(
        name = "list_draft_comments",
        description = "List draft comments on a Gerrit change"
    )]
    pub async fn list_draft_comments(
        &self,
        Parameters(params): Parameters<ListDraftCommentsParams>,
    ) -> CallToolResult {
        match self.repo.list_drafts(&params.change_id).await {
            Ok(drafts) => {
                if drafts.is_empty() {
                    return self.text("No draft comments.".to_string());
                }
                let mut lines = Vec::new();
                for (file, file_drafts) in &drafts {
                    lines.push(format!("File: {}", file));
                    for d in file_drafts {
                        let preview = if d.message.len() > 120 {
                            format!("{}...", &d.message[..120])
                        } else {
                            d.message.clone()
                        };
                        let line_str = d
                            .line
                            .map(|l| format!("L{}", l))
                            .unwrap_or_else(|| "N/A".to_string());
                        lines.push(format!("  {} [{}] {}", line_str, d.id, preview));
                    }
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to list draft comments: {e}")),
        }
    }

    #[tool(
        name = "get_most_recent_cl",
        description = "Get the most recent change from a Gerrit user"
    )]
    pub async fn get_most_recent_cl(
        &self,
        Parameters(params): Parameters<GetMostRecentClParams>,
    ) -> CallToolResult {
        let query = format!("owner:{}", params.user);
        match self.repo.query_changes(&query, Some(1), &[]).await {
            Ok(changes) => {
                if changes.is_empty() {
                    self.text(format!("No changes found for user: {}", params.user))
                } else {
                    let c = &changes[0];
                    self.text(format!("{}_{}: {}", c._number, c.updated, c.subject))
                }
            }
            Err(e) => self.error(format!("Failed to query most recent CL: {e}")),
        }
    }

    #[tool(
        name = "get_bugs_from_cl",
        description = "Extract bug references from a Gerrit change"
    )]
    pub async fn get_bugs_from_cl(
        &self,
        Parameters(params): Parameters<GetBugsFromClParams>,
    ) -> CallToolResult {
        match self.repo.get_commit(&params.change_id).await {
            Ok(commit) => {
                let bugs = extract_bugs(&commit.message);
                if bugs.is_empty() {
                    self.text("No bugs found in commit message.".to_string())
                } else {
                    self.text(format!("Bugs: {}", bugs.join(", ")))
                }
            }
            Err(e) => self.error(format!("Failed to get bugs from CL: {e}")),
        }
    }

    #[tool(
        name = "suggest_reviewers",
        description = "Suggest reviewers for a Gerrit change"
    )]
    pub async fn suggest_reviewers(
        &self,
        Parameters(params): Parameters<SuggestReviewersParams>,
    ) -> CallToolResult {
        let exclude_groups = params.exclude_groups.unwrap_or(false);
        match self
            .repo
            .suggest_reviewers(
                &params.change_id,
                &params.query,
                params.limit,
                exclude_groups,
                params.reviewer_state.as_deref(),
            )
            .await
        {
            Ok(suggestions) => {
                if suggestions.is_empty() {
                    return self.text("No reviewer suggestions found.".to_string());
                }
                let mut lines = Vec::new();
                for s in &suggestions {
                    if let Some(ref account) = s.account {
                        let name = account.name.as_deref().unwrap_or("unknown");
                        let email = account.email.as_deref().unwrap_or("no email");
                        lines.push(format!("Account: {} <{}>", name, email));
                    }
                    if let Some(ref group) = s.group {
                        lines.push(format!("Group: {}", group.name));
                    }
                }
                self.text(lines.join("\n"))
            }
            Err(e) => self.error(format!("Failed to suggest reviewers: {e}")),
        }
    }

    #[tool(
        name = "changes_submitted_together",
        description = "List changes submitted together with this one"
    )]
    pub async fn changes_submitted_together(
        &self,
        Parameters(params): Parameters<ChangesSubmittedTogetherParams>,
    ) -> CallToolResult {
        let extra = params.options.unwrap_or_default();
        match self
            .repo
            .changes_submitted_together(&params.change_id, &extra)
            .await
        {
            Ok(submitted) => {
                let mut lines = Vec::new();
                for c in &submitted.changes {
                    lines.push(format!("{}_{}: {}", c._number, c.updated, c.subject));
                }
                if submitted.non_visible_changes > 0 {
                    lines.push(format!(
                        "({} changes not visible)",
                        submitted.non_visible_changes
                    ));
                }
                if lines.is_empty() {
                    self.text("No changes submitted together.".to_string())
                } else {
                    self.text(lines.join("\n"))
                }
            }
            Err(e) => self.error(format!("Failed to get changes submitted together: {e}")),
        }
    }

    #[tool(name = "create_change", description = "Create a new change in Gerrit")]
    pub async fn create_change(
        &self,
        Parameters(params): Parameters<CreateChangeParams>,
    ) -> CallToolResult {
        let payload = CreateChangeRequest {
            project: params.project,
            branch: params.branch,
            subject: params.subject,
            topic: params.topic,
            status: params.status,
            is_private: None,
            work_in_progress: None,
            base_change: None,
            new_branch: None,
        };
        match self.repo.create_change(&payload).await {
            Ok(change) => self.text(format!(
                "Created change {}_{}: {}",
                change._number, change.updated, change.subject
            )),
            Err(e) => self.error(format!("Failed to create change: {e}")),
        }
    }

    #[tool(
        name = "add_reviewer",
        description = "Add a reviewer to a Gerrit change"
    )]
    pub async fn add_reviewer(
        &self,
        Parameters(params): Parameters<AddReviewerParams>,
    ) -> CallToolResult {
        let state = params.state.as_deref().unwrap_or(REVIEWER_STATE_REVIEWER);
        if state != REVIEWER_STATE_REVIEWER && state != REVIEWER_STATE_CC {
            return self.error(format!("Invalid state '{}': must be REVIEWER or CC", state));
        }
        let payload = AddReviewerRequest {
            reviewer: params.reviewer,
            confirmed: Some(true),
            state: Some(state.to_string()),
            notify: None,
        };
        match self.repo.add_reviewer(&params.change_id, &payload).await {
            Ok(result) => {
                if let Some(ref err_msg) = result.error {
                    return self.error(format!("Failed to add reviewer: {}", err_msg));
                }
                if result.reviewers.is_empty() {
                    self.text("Reviewer added successfully.".to_string())
                } else {
                    let reviewer = &result.reviewers[0];
                    let email = reviewer.email.as_deref().unwrap_or("unknown");
                    self.text(format!("Added {} as {}.", email, state))
                }
            }
            Err(e) => self.error(format!("Failed to add reviewer: {e}")),
        }
    }

    #[tool(
        name = "set_ready_for_review",
        description = "Mark a Gerrit change as ready for review"
    )]
    pub async fn set_ready_for_review(
        &self,
        Parameters(params): Parameters<SetReadyParams>,
    ) -> CallToolResult {
        match self.repo.set_ready(&params.change_id).await {
            Ok(()) => self.text(format!(
                "Change {} marked as ready for review.",
                params.change_id
            )),
            Err(e) => self.error(format!("Failed to set ready for review: {e}")),
        }
    }

    #[tool(
        name = "set_work_in_progress",
        description = "Mark a Gerrit change as work-in-progress"
    )]
    pub async fn set_work_in_progress(
        &self,
        Parameters(params): Parameters<SetWipParams>,
    ) -> CallToolResult {
        let payload = WipRequest {
            message: params.message,
        };
        match self.repo.set_wip(&params.change_id, &payload).await {
            Ok(()) => self.text(format!(
                "Change {} marked as work-in-progress.",
                params.change_id
            )),
            Err(e) => self.error(format!("Failed to set WIP: {e}")),
        }
    }

    #[tool(name = "set_topic", description = "Set the topic for a Gerrit change")]
    pub async fn set_topic(
        &self,
        Parameters(params): Parameters<SetTopicParams>,
    ) -> CallToolResult {
        let payload = TopicRequest {
            topic: params.topic.clone(),
        };
        match self.repo.set_topic(&params.change_id, &payload).await {
            Ok(Some(_response)) => self.text(format!("Topic set to '{}'.", params.topic)),
            Ok(None) => self.text("Topic deleted (empty response).".to_string()),
            Err(e) => self.error(format!("Failed to set topic: {e}")),
        }
    }

    #[tool(name = "abandon_change", description = "Abandon a Gerrit change")]
    pub async fn abandon_change(
        &self,
        Parameters(params): Parameters<AbandonChangeParams>,
    ) -> CallToolResult {
        let payload = AbandonRequest {
            message: params.message,
            notify: None,
        };
        match self.repo.abandon_change(&params.change_id, &payload).await {
            Ok(change) => self.text(format!(
                "Change {} abandoned: {}",
                change._number, change.subject
            )),
            Err(e) => self.error(format!("Failed to abandon change: {e}")),
        }
    }

    #[tool(name = "revert_change", description = "Revert a Gerrit change")]
    pub async fn revert_change(
        &self,
        Parameters(params): Parameters<RevertChangeParams>,
    ) -> CallToolResult {
        match self
            .repo
            .revert_change(&params.change_id, params.message.as_deref())
            .await
        {
            Ok(change) => self.text(format!(
                "Revert created: {}_{}: {}",
                change._number, change.updated, change.subject
            )),
            Err(e) => self.error(format!("Failed to revert change: {e}")),
        }
    }

    #[tool(name = "revert_submission", description = "Revert a Gerrit submission")]
    pub async fn revert_submission(
        &self,
        Parameters(params): Parameters<RevertSubmissionParams>,
    ) -> CallToolResult {
        match self
            .repo
            .revert_submission(&params.change_id, params.message.as_deref())
            .await
        {
            Ok(changes) => {
                let mut lines = Vec::new();
                for c in &changes {
                    lines.push(format!("{}_{}: {}", c._number, c.updated, c.subject));
                }
                if lines.is_empty() {
                    self.text("No revert changes created.".to_string())
                } else {
                    self.text(format!("Revert changes created:\n{}", lines.join("\n")))
                }
            }
            Err(e) => self.error(format!("Failed to revert submission: {e}")),
        }
    }

    #[tool(
        name = "post_review_comment",
        description = "Post a review comment on a Gerrit change"
    )]
    pub async fn post_review_comment(
        &self,
        Parameters(params): Parameters<PostReviewCommentParams>,
    ) -> CallToolResult {
        let comment = CommentInput {
            id: None,
            path: Some(params.file_path.clone()),
            side: None,
            line: Some(params.line_number),
            range: None,
            in_reply_to: None,
            updated: None,
            message: params.message.clone(),
            tag: None,
            unresolved: params.unresolved,
        };
        let mut comments_map: BTreeMap<String, Vec<CommentInput>> = BTreeMap::new();
        comments_map.insert(params.file_path.clone(), vec![comment]);
        let batch = CommentBatchInput {
            comments: Some(comments_map),
            drafts: None,
            omit_duplicate_comments: None,
            notify: None,
        };
        match self.repo.post_review(&params.change_id, &batch).await {
            Ok(()) => self.text("Review comment posted.".to_string()),
            Err(e) => self.error(format!("Failed to post review comment: {e}")),
        }
    }

    #[tool(
        name = "post_draft_comment",
        description = "Post a draft comment on a Gerrit change"
    )]
    pub async fn post_draft_comment(
        &self,
        Parameters(params): Parameters<PostDraftCommentParams>,
    ) -> CallToolResult {
        let mut message = params.message.clone();
        let suggestion = if message.starts_with("suggestion:") {
            let s = message
                .strip_prefix("suggestion:")
                .unwrap_or("")
                .trim()
                .to_string();
            message = s;
            params.suggestion.or(Some(message.clone()))
        } else {
            params.suggestion.clone()
        };

        let line = if params.start_line.is_some()
            && params.start_character.is_some()
            && params.end_line.is_some()
            && params.end_character.is_some()
        {
            params.end_line
        } else {
            Some(params.line_number)
        };

        let draft = DraftInput {
            path: params.file_path.clone(),
            line,
            message: message.clone(),
            side: None,
            parent: None,
            in_reply_to: params.in_reply_to,
            updated: None,
            tag: None,
        };
        match self.repo.post_draft(&params.change_id, &draft).await {
            Ok(draft_id) => {
                let mut msg = format!("Draft comment posted (id: {}).", draft_id);
                if let Some(s) = suggestion {
                    msg.push_str(&format!("\nSuggestion: {}", s));
                }
                self.text(msg)
            }
            Err(e) => self.error(format!("Failed to post draft comment: {e}")),
        }
    }

    #[tool(
        name = "delete_draft_comment",
        description = "Delete a specific draft comment on a Gerrit change"
    )]
    pub async fn delete_draft_comment(
        &self,
        Parameters(params): Parameters<DeleteDraftCommentParams>,
    ) -> CallToolResult {
        match self
            .repo
            .delete_draft(&params.change_id, &params.draft_id)
            .await
        {
            Ok(()) => self.text(format!("Draft {} deleted.", params.draft_id)),
            Err(e) => self.error(format!("Failed to delete draft: {e}")),
        }
    }

    #[tool(
        name = "delete_draft_comments",
        description = "Delete all draft comments on a Gerrit change"
    )]
    pub async fn delete_draft_comments(
        &self,
        Parameters(params): Parameters<DeleteDraftCommentsParams>,
    ) -> CallToolResult {
        let drafts = match self.repo.list_drafts(&params.change_id).await {
            Ok(d) => d,
            Err(e) => return self.error(format!("Failed to list drafts: {e}")),
        };

        let mut draft_ids: Vec<String> = Vec::new();
        for file_drafts in drafts.values() {
            for d in file_drafts {
                draft_ids.push(d.id.clone());
            }
        }

        let mut errors = Vec::new();
        let mut deleted = 0;
        for id in &draft_ids {
            match self.repo.delete_draft(&params.change_id, id).await {
                Ok(()) => deleted += 1,
                Err(e) => errors.push(format!("  {}: {}", id, e)),
            }
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "Deleted {} of {} draft comments.",
            deleted,
            draft_ids.len()
        ));
        if !errors.is_empty() {
            lines.push("Errors:".to_string());
            lines.extend(errors);
        }
        self.text(lines.join("\n"))
    }

    #[tool(
        name = "publish_drafts",
        description = "Publish draft comments on a Gerrit change"
    )]
    pub async fn publish_drafts(
        &self,
        Parameters(params): Parameters<PublishDraftsParams>,
    ) -> CallToolResult {
        let payload = PublishDraftsRequest { notify: None };
        match self.repo.publish_drafts(&params.change_id, &payload).await {
            Ok(()) => self.text("All drafts published.".to_string()),
            Err(e) => self.error(format!("Failed to publish drafts: {e}")),
        }
    }

    #[tool(
        name = "cherry_pick_change",
        description = "Cherry-pick a Gerrit change to a destination branch"
    )]
    pub async fn cherry_pick_change(
        &self,
        Parameters(params): Parameters<CherryPickChangeParams>,
    ) -> CallToolResult {
        let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);
        let payload = CherryPickRequest {
            message: params.message,
            destination: params.destination,
            parent: None,
            base: None,
            notify: None,
        };
        match self
            .repo
            .cherry_pick(&params.change_id, revision, &payload)
            .await
        {
            Ok(result) => self.text(format!(
                "Successfully cherry-picked to new CL: {}",
                result._number
            )),
            Err(e) => self.error(format!("Failed to cherry-pick change: {e}")),
        }
    }

    #[tool(
        name = "cherry_pick_chain",
        description = "Cherry-pick a chain of Gerrit changes to a destination branch"
    )]
    pub async fn cherry_pick_chain(
        &self,
        Parameters(params): Parameters<CherryPickChainParams>,
    ) -> CallToolResult {
        let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);

        let related = match self.repo.get_related(&params.change_id, revision).await {
            Ok(r) => r,
            Err(e) => return self.error(format!("Failed to get related changes: {e}")),
        };

        if related.is_empty() {
            return self.text("No related changes found.".to_string());
        }

        let reversed: Vec<&RelatedChange> = related.iter().rev().collect();
        let mut results: Vec<String> = Vec::new();
        let mut base: Option<String> = None;

        for rc in &reversed {
            let cp_payload = CherryPickRequest {
                message: None,
                destination: params.destination.clone(),
                parent: None,
                base: base.clone(),
                notify: None,
            };

            let change_id_str = rc._change_number.to_string();
            let revision_str = rc._revision_number.to_string();

            match self
                .repo
                .cherry_pick(&change_id_str, &revision_str, &cp_payload)
                .await
            {
                Ok(result) => {
                    let new_number = result._number;
                    let new_id = new_number.to_string();
                    results.push(format!(
                        "Cherry-picked {} (rev {}) -> new CL: {}",
                        change_id_str, revision_str, new_number
                    ));

                    let base_opts = vec![
                        GERRIT_OPTION_CURRENT_REVISION.to_string(),
                        GERRIT_OPTION_CURRENT_COMMIT.to_string(),
                    ];
                    match self.repo.get_change_detail(&new_id, &base_opts).await {
                        Ok(detail) => {
                            if let Some(ref rev_key) = detail.current_revision
                                && let Some(rev_info) = detail.revisions.get(rev_key)
                                && let Some(ref commit) = rev_info.commit
                            {
                                base = Some(commit.message.clone());
                            }
                            if base.is_none() {
                                base = Some(new_id);
                            }
                        }
                        Err(_) => {
                            base = Some(new_id);
                        }
                    }
                }
                Err(e) => {
                    if !results.is_empty() {
                        results.push(format!(
                            "Partial failure at change {} (rev {}): {}",
                            change_id_str, revision_str, e
                        ));
                        let mut out = results.join("\n");
                        out.push_str("\nSome changes were cherry-picked successfully.");
                        return self.text(out);
                    }
                    return self.error(format!(
                        "Failed to cherry-pick change {} (rev {}): {}",
                        change_id_str, revision_str, e
                    ));
                }
            }
        }

        self.text(format!(
            "Successfully cherry-picked chain of {} changes:\n{}",
            reversed.len(),
            results.join("\n")
        ))
    }

    #[tool(
        name = "submit_change",
        description = "Submit a Gerrit change for merge"
    )]
    pub async fn submit_change(
        &self,
        Parameters(params): Parameters<SubmitChangeParams>,
    ) -> CallToolResult {
        let payload = SubmitRequest {
            wait_for_merge: params.wait_for_merge,
            on_behalf_of: None,
            notify: None,
        };
        match self.repo.submit_change(&params.change_id, &payload).await {
            Ok(result) => self.text(format!(
                "Successfully submitted change {}: status={}",
                result._number, result.status
            )),
            Err(e) => self.error(format!("Failed to submit change: {e}")),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use gerrit_core::domain::MockGerritRepository;

    fn extract_text(result: CallToolResult) -> String {
        if let Some(text) = result.content.iter().find_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        }) {
            return text;
        }
        String::new()
    }

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
        mock.push_query_changes_result(Ok(vec![make_change(12345, "Test Subject")]));
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
            gerrit_base_url: "https://g.example.com".to_string(),
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
        }));

        let server = GerritServer::new(mock);

        let params = CherryPickChainParams {
            change_id: "2".to_string(),
            destination: "main".to_string(),
            revision_id: None,
            keep_reviewers: None,
            allow_conflicts: None,
            allow_empty: None,
            gerrit_base_url: "https://g.example.com".to_string(),
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
            gerrit_base_url: "https://g.example.com".to_string(),
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
}
