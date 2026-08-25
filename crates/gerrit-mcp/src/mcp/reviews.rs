// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Reviewer, file diff, and cherry-pick MCP tool implementations.

use gerrit_core::domain::*;
use rmcp::model::CallToolResult;

use crate::mcp::GerritServer;
use crate::mcp::tools::*;
use crate::mcp::{
    DEFAULT_REVISION, GERRIT_OPTION_CURRENT_COMMIT, GERRIT_OPTION_CURRENT_REVISION,
    REVIEWER_STATE_REVIEWER,
};

pub async fn list_change_files<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: ListChangeFilesParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.list_files(&params.change_id).await;
    match result {
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
                server.text("No files found.".to_string())
            } else {
                server.text(lines.join("\n"))
            }
        }
        Err(e) => server.error(format!("Failed to list change files: {e}")),
    }
}

pub async fn get_file_diff<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: GetFileDiffParams,
) -> CallToolResult {
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo.get_diff(&params.change_id, &params.file_path).await;
    match result {
        Ok(text) => server.text(text),
        Err(e) => server.error(format!("Failed to get file diff: {e}")),
    }
}

pub async fn suggest_reviewers<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: SuggestReviewersParams,
) -> CallToolResult {
    let exclude_groups = params.exclude_groups.unwrap_or(false);
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let result = repo
        .suggest_reviewers(
            &params.change_id,
            &params.query,
            params.limit,
            exclude_groups,
            params.reviewer_state.as_deref(),
        )
        .await;
    match result {
        Ok(suggestions) => {
            if suggestions.is_empty() {
                return server.text("No reviewer suggestions found.".to_string());
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
            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to suggest reviewers: {e}")),
    }
}

pub async fn add_reviewer<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: AddReviewerParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("add reviewer") {
        return r;
    }
    let state = params.state.as_deref().unwrap_or(REVIEWER_STATE_REVIEWER);
    if state != REVIEWER_STATE_REVIEWER && state != "CC" {
        return server.error(format!("Invalid state '{}': must be REVIEWER or CC", state));
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let payload = AddReviewerRequest {
        reviewer: params.reviewer,
        confirmed: params.confirmed,
        state: Some(state.to_string()),
        notify: None,
    };
    match repo.add_reviewer(&params.change_id, &payload).await {
        Ok(result) => {
            if let Some(ref err_msg) = result.error {
                return server.error(format!("Failed to add reviewer: {}", err_msg));
            }
            if result.reviewers.is_empty() {
                server.text("Reviewer added successfully.".to_string())
            } else {
                let reviewer = &result.reviewers[0];
                let email = reviewer.email.as_deref().unwrap_or("unknown");
                server.text(format!("Added {} as {}.", email, state))
            }
        }
        Err(e) => server.error(format!("Failed to add reviewer: {e}")),
    }
}

pub async fn cherry_pick_change<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: CherryPickChangeParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("cherry-pick change") {
        return r;
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);
    let payload = CherryPickRequest {
        message: params.message,
        destination: params.destination,
        parent: None,
        base: None,
        notify: None,
        keep_reviewers: params.keep_reviewers,
        allow_conflicts: params.allow_conflicts,
        allow_empty: params.allow_empty,
    };
    match repo
        .cherry_pick(&params.change_id, revision, &payload)
        .await
    {
        Ok(result) => server.text(format!(
            "Successfully cherry-picked to new CL: {}",
            result._number
        )),
        Err(e) => server.error(format!("Failed to cherry-pick change: {e}")),
    }
}

pub async fn cherry_pick_chain<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: CherryPickChainParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("cherry-pick chain") {
        return r;
    }
    let repo = match server.resolve_repo(params.gerrit_base_url.as_deref()) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let revision = params.revision_id.as_deref().unwrap_or(DEFAULT_REVISION);

    let related = match repo.get_related(&params.change_id, revision).await {
        Ok(r) => r,
        Err(e) => return server.error(format!("Failed to get related changes: {e}")),
    };

    if related.is_empty() {
        return server.text("No related changes found.".to_string());
    }

    let reversed: Vec<&RelatedChange> = related.iter().rev().collect();
    let mut results: Vec<String> = Vec::new();
    let mut base: Option<String> = None;

    let partial = |results: Vec<String>, reason: String| {
        let mut out = results.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&reason);
        out.push_str("\nSome changes were cherry-picked successfully.");
        server.text(out)
    };

    let total = reversed.len();

    for (idx, rc) in reversed.iter().enumerate() {
        let cp_payload = CherryPickRequest {
            message: None,
            destination: params.destination.clone(),
            parent: None,
            base: base.clone(),
            notify: None,
            keep_reviewers: params.keep_reviewers,
            allow_conflicts: params.allow_conflicts,
            allow_empty: params.allow_empty,
        };

        let change_id_str = rc._change_number.to_string();
        let revision_str = rc._revision_number.to_string();
        let need_base = idx + 1 < total;

        match repo
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

                if !need_base {
                    // No later cherry-pick has to stack on this result, so the
                    // base SHA of the newly created change is not required.
                    continue;
                }

                let base_opts = vec![
                    GERRIT_OPTION_CURRENT_REVISION.to_string(),
                    GERRIT_OPTION_CURRENT_COMMIT.to_string(),
                ];
                match repo.get_change_detail(&new_id, &base_opts).await {
                    Ok(detail) => match detail.current_revision {
                        Some(rev_key) => base = Some(rev_key),
                        None => {
                            return partial(
                                results,
                                format!(
                                    "Partial failure at change {} (rev {}): cannot determine the new revision SHA to chain onto it",
                                    change_id_str, revision_str
                                ),
                            );
                        }
                    },
                    Err(e) => {
                        return partial(
                            results,
                            format!(
                                "Partial failure at change {} (rev {}): cannot read the new change to determine its revision SHA ({e})",
                                change_id_str, revision_str
                            ),
                        );
                    }
                }
            }
            Err(e) => {
                if !results.is_empty() {
                    return partial(
                        results,
                        format!(
                            "Partial failure at change {} (rev {}): {}",
                            change_id_str, revision_str, e
                        ),
                    );
                }
                return server.error(format!(
                    "Failed to cherry-pick change {} (rev {}): {}",
                    change_id_str, revision_str, e
                ));
            }
        }
    }

    server.text(format!(
        "Successfully cherry-picked chain of {} changes:\n{}",
        reversed.len(),
        results.join("\n")
    ))
}
