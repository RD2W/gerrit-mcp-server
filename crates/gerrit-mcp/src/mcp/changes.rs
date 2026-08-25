// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Change querying and management MCP tool implementations.

use std::collections::BTreeMap;

use gerrit_core::domain::*;
use rmcp::model::CallToolResult;

use crate::health::metrics;
use crate::mcp::GerritServer;
use crate::mcp::tools::*;
use crate::mcp::{
    DEFAULT_REVISION, DEFAULT_STATUS_MERGED, GERRIT_OPTION_CURRENT_COMMIT,
    GERRIT_OPTION_CURRENT_REVISION, GERRIT_OPTION_DETAILED_LABELS, REVIEWER_STATE_REVIEWER,
    extract_bugs, format_changes_output, sort_by_date,
};

pub async fn query_changes<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: QueryChangesParams,
) -> CallToolResult {
    let opts = params.options.unwrap_or_default();
    metrics().record_query();

    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.query_changes(&params.query, params.limit, &opts).await;

    match result {
        Ok(mut changes) => {
            if changes.is_empty() {
                return server.text(format!("No changes found for query: {}", params.query));
            }
            sort_by_date(&mut changes);
            server.text(format_changes_output(&changes))
        }
        Err(e) => server.error(format!("Failed to query changes: {e}")),
    }
}

pub async fn query_changes_by_date_and_filters<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: QueryChangesByDateParams,
) -> CallToolResult {
    let start_date = match chrono::NaiveDate::parse_from_str(&params.start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => return server.error(format!("Invalid start_date format: {e}")),
    };
    let start_str = start_date.format("%Y-%m-%d").to_string();

    let end_date = match chrono::NaiveDate::parse_from_str(&params.end_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => return server.error(format!("Invalid end_date format: {e}")),
    };
    let end_plus_one = end_date
        .succ_opt()
        .unwrap_or(end_date)
        .format("%Y-%m-%d")
        .to_string();

    let status = params.status.as_deref().unwrap_or(DEFAULT_STATUS_MERGED);
    let mut query = format!(
        "status:{} after:{} before:{}",
        status, start_str, end_plus_one
    );

    if let Some(ref project) = params.project {
        query.push_str(&format!(" project:{}", project));
    }
    if let Some(ref msg) = params.message_substring {
        let escaped = msg.replace('"', "\\\"");
        query.push_str(&format!(" message:\"{}\"", escaped));
    }

    let opts = Vec::new();
    metrics().record_query();
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.query_changes(&query, params.limit, &opts).await;
    match result {
        Ok(mut changes) => {
            if changes.is_empty() {
                return server.text(format!("No changes found for query: {}", query));
            }
            sort_by_date(&mut changes);
            server.text(format_changes_output(&changes))
        }
        Err(e) => server.error(format!("Failed to query changes by date: {e}")),
    }
}

pub async fn get_change_details<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetChangeDetailsParams,
) -> CallToolResult {
    let base = &[
        GERRIT_OPTION_CURRENT_REVISION,
        GERRIT_OPTION_CURRENT_COMMIT,
        GERRIT_OPTION_DETAILED_LABELS,
    ];
    let extra = params.options.unwrap_or_default();
    let opts = GerritServer::<R>::merge_options(base, &extra);

    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.get_change_detail(&params.change_id, &opts).await;
    match result {
        Ok(detail) => {
            let mut lines = Vec::new();
            lines.push(format!("Subject: {}", detail.subject));
            lines.push(format!(
                "Owner: {} <{}>",
                detail.owner.name.as_deref().unwrap_or("unknown"),
                detail.owner.email.as_deref().unwrap_or("no email")
            ));
            lines.push(format!("Status: {}", detail.status));

            if let Some(ref topic) = detail.topic {
                lines.push(format!("Topic: {}", topic));
            }

            let commit_msg = detail
                .current_revision
                .as_ref()
                .and_then(|k| detail.revisions.get(k))
                .and_then(|r| r.commit.as_ref())
                .map(|c| c.message.clone())
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

            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to get change details: {e}")),
    }
}

pub async fn get_commit_message<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetCommitMessageParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.get_commit_message(&params.change_id).await;
    match result {
        Ok(msg) => server.text(msg.full_message),
        Err(e) => server.error(format!("Failed to get commit message: {e}")),
    }
}

pub async fn get_most_recent_cl<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetMostRecentClParams,
) -> CallToolResult {
    let query = format!("owner:{}", params.user);
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.query_changes(&query, Some(1), &[]).await;
    match result {
        Ok(changes) => {
            if changes.is_empty() {
                server.text(format!("No changes found for user: {}", params.user))
            } else {
                let c = &changes[0];
                server.text(format!("{}_{}: {}", c._number, c.updated, c.subject))
            }
        }
        Err(e) => server.error(format!("Failed to query most recent CL: {e}")),
    }
}

pub async fn get_bugs_from_cl<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetBugsFromClParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.get_commit(&params.change_id).await;
    match result {
        Ok(commit) => {
            let bugs = extract_bugs(&commit.message);
            if bugs.is_empty() {
                server.text("No bugs found in commit message.".to_string())
            } else {
                server.text(format!("Bugs: {}", bugs.join(", ")))
            }
        }
        Err(e) => server.error(format!("Failed to get bugs from CL: {e}")),
    }
}

pub async fn get_revision_commit<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetRevisionCommitParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);
    let result = repo.get_revision_commit(&params.change_id, revision).await;
    match result {
        Ok(c) => {
            let mut lines = Vec::new();
            lines.push(format!("Commit: {}", c.commit));
            lines.push(format!(
                "Author: {} <{}> {}",
                c.author.name, c.author.email, c.author.date
            ));
            lines.push(format!(
                "Committer: {} <{}> {}",
                c.committer.name, c.committer.email, c.committer.date
            ));
            if !c.parents.is_empty() {
                let parents: Vec<String> = c.parents.iter().map(|p| p.commit.clone()).collect();
                lines.push(format!("Parents: {}", parents.join(", ")));
            }
            lines.push(String::new());
            lines.push(format!("Subject: {}", c.subject));
            lines.push(String::new());
            lines.push(c.message.clone());
            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to get revision commit: {e}")),
    }
}

pub async fn get_related_changes<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetRelatedChangesParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);
    let result = repo.get_related(&params.change_id, revision).await;
    match result {
        Ok(related) => {
            if related.is_empty() {
                return server.text(format!(
                    "No related changes found for {}.",
                    params.change_id
                ));
            }
            let mut lines = vec![format!("Related changes for {}:", params.change_id)];
            for rc in &related {
                let subject = rc.subject.clone().unwrap_or_else(|| "no subject".into());
                let status = rc.status.clone().unwrap_or_default();
                lines.push(format!(
                    "- {} ({}): {} [{}]",
                    rc._change_number, rc._revision_number, subject, status
                ));
            }
            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to get related changes: {e}")),
    }
}

pub async fn get_git_parent_changes<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetGitParentChangesParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let query = format!("parentof:{}", params.change_id);
    let limit = params.limit.unwrap_or(10);
    let result = repo.query_changes(&query, Some(limit), &[]).await;
    match result {
        Ok(changes) => {
            if changes.is_empty() {
                server.text(format!("No parent changes found for {}.", params.change_id))
            } else {
                server.text(format_changes_output(&changes))
            }
        }
        Err(e) => server.error(format!("Failed to get parent changes: {e}")),
    }
}

pub async fn changes_submitted_together<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: ChangesSubmittedTogetherParams,
) -> CallToolResult {
    let extra = params.options.unwrap_or_default();
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo
        .changes_submitted_together(&params.change_id, &extra)
        .await;
    match result {
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
                server.text("No changes submitted together.".to_string())
            } else {
                server.text(lines.join("\n"))
            }
        }
        Err(e) => server.error(format!("Failed to get changes submitted together: {e}")),
    }
}

pub async fn create_change<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: CreateChangeParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("create change") {
        return r;
    }
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
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.create_change(&payload).await {
        Ok(change) => server.text(format!(
            "Created change {}_{}: {}",
            change._number, change.updated, change.subject
        )),
        Err(e) => server.error(format!("Failed to create change: {e}")),
    }
}

pub async fn set_ready_for_review<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SetReadyParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("set ready for review") {
        return r;
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.set_ready(&params.change_id).await {
        Ok(()) => server.text(format!(
            "Change {} marked as ready for review.",
            params.change_id
        )),
        Err(e) => server.error(format!("Failed to set ready for review: {e}")),
    }
}

pub async fn set_work_in_progress<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SetWipParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("set work-in-progress") {
        return r;
    }
    let payload = WipRequest {
        message: params.message,
    };
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.set_wip(&params.change_id, &payload).await {
        Ok(()) => server.text(format!(
            "Change {} marked as work-in-progress.",
            params.change_id
        )),
        Err(e) => server.error(format!("Failed to set WIP: {e}")),
    }
}

pub async fn set_topic<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SetTopicParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("set topic") {
        return r;
    }
    let payload = TopicRequest {
        topic: params.topic.clone(),
    };
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.set_topic(&params.change_id, &payload).await {
        Ok(Some(_response)) => server.text(format!("Topic set to '{}'.", params.topic)),
        Ok(None) => server.text("Topic deleted (empty response).".to_string()),
        Err(e) => server.error(format!("Failed to set topic: {e}")),
    }
}

fn format_labels(labels: &BTreeMap<String, i32>) -> String {
    labels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub async fn set_labels<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SetLabelsParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("set labels") {
        return r;
    }
    let payload = ReviewInput {
        message: params.message,
        labels: Some(params.labels.clone()),
        comments: None,
        tag: None,
        drafts: None,
        notify: None,
        omit_duplicate_comments: None,
    };
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.set_labels(&params.change_id, &payload).await;

    match result {
        Ok(()) => server.text(format!(
            "Labels set on change {}: {}",
            params.change_id,
            format_labels(&params.labels)
        )),
        Err(e) => server.error(format!("Failed to set labels: {e}")),
    }
}

pub async fn abandon_change<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: AbandonChangeParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("abandon change") {
        return r;
    }
    let payload = AbandonRequest {
        message: params.message,
        notify: None,
    };
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.abandon_change(&params.change_id, &payload).await {
        Ok(change) => server.text(format!(
            "Change {} abandoned: {}",
            change._number, change.subject
        )),
        Err(e) => server.error(format!("Failed to abandon change: {e}")),
    }
}

pub async fn revert_change<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: RevertChangeParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("revert change") {
        return r;
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo
        .revert_change(&params.change_id, params.message.as_deref())
        .await
    {
        Ok(change) => server.text(format!(
            "Revert created: {}_{}: {}",
            change._number, change.updated, change.subject
        )),
        Err(e) => server.error(format!("Failed to revert change: {e}")),
    }
}

pub async fn revert_submission<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: RevertSubmissionParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("revert submission") {
        return r;
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo
        .revert_submission(&params.change_id, params.message.as_deref())
        .await
    {
        Ok(changes) => {
            let mut lines = Vec::new();
            for c in &changes {
                lines.push(format!("{}_{}: {}", c._number, c.updated, c.subject));
            }
            if lines.is_empty() {
                server.text("No revert changes created.".to_string())
            } else {
                server.text(format!("Revert changes created:\n{}", lines.join("\n")))
            }
        }
        Err(e) => server.error(format!("Failed to revert submission: {e}")),
    }
}

pub async fn submit_change<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SubmitChangeParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("submit change") {
        return r;
    }
    let payload = SubmitRequest {
        wait_for_merge: params.wait_for_merge,
        on_behalf_of: None,
        notify: None,
    };
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match repo.submit_change(&params.change_id, &payload).await {
        Ok(result) => server.text(format!(
            "Successfully submitted change {}: status={}",
            result._number, result.status
        )),
        Err(e) => server.error(format!("Failed to submit change: {e}")),
    }
}
