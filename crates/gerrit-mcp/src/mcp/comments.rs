// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Comment MCP tool implementations.

use std::collections::BTreeMap;

use gerrit_core::domain::*;
use rmcp::model::CallToolResult;

use crate::mcp::GerritServer;
use crate::mcp::tools::*;

pub async fn list_change_comments<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: ListChangeCommentsParams,
) -> CallToolResult {
    let result = if let Some(ref url) = params.gerrit_base_url {
        match server.resolve_client(Some(url)) {
            Ok(client) => client.list_comments(&params.change_id).await,
            Err(e) => return server.error(e),
        }
    } else {
        server.repo.list_comments(&params.change_id).await
    };
    match result {
        Ok(comments) => {
            if comments.is_empty() {
                return server.text("No comments.".to_string());
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
            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to list comments: {e}")),
    }
}

pub async fn list_draft_comments<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: ListDraftCommentsParams,
) -> CallToolResult {
    let result = if let Some(ref url) = params.gerrit_base_url {
        match server.resolve_client(Some(url)) {
            Ok(client) => client.list_drafts(&params.change_id).await,
            Err(e) => return server.error(e),
        }
    } else {
        server.repo.list_drafts(&params.change_id).await
    };
    match result {
        Ok(drafts) => {
            if drafts.is_empty() {
                return server.text("No draft comments.".to_string());
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
            server.text(lines.join("\n"))
        }
        Err(e) => server.error(format!("Failed to list draft comments: {e}")),
    }
}

pub async fn post_review_comment<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: PostReviewCommentParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("post review comment") {
        return r;
    }
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
    match server.repo.post_review(&params.change_id, &batch).await {
        Ok(()) => server.text("Review comment posted.".to_string()),
        Err(e) => server.error(format!("Failed to post review comment: {e}")),
    }
}

pub async fn post_draft_comment<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: PostDraftCommentParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("post draft comment") {
        return r;
    }
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
    match server.repo.post_draft(&params.change_id, &draft).await {
        Ok(draft_id) => {
            let mut msg = format!("Draft comment posted (id: {}).", draft_id);
            if let Some(s) = suggestion {
                msg.push_str(&format!("\nSuggestion: {}", s));
            }
            server.text(msg)
        }
        Err(e) => server.error(format!("Failed to post draft comment: {e}")),
    }
}

pub async fn delete_draft_comment<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: DeleteDraftCommentParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("delete draft comment") {
        return r;
    }
    match server
        .repo
        .delete_draft(&params.change_id, &params.draft_id)
        .await
    {
        Ok(()) => server.text(format!("Draft {} deleted.", params.draft_id)),
        Err(e) => server.error(format!("Failed to delete draft: {e}")),
    }
}

pub async fn delete_draft_comments<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: DeleteDraftCommentsParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("delete draft comments") {
        return r;
    }
    let drafts = match server.repo.list_drafts(&params.change_id).await {
        Ok(d) => d,
        Err(e) => return server.error(format!("Failed to list drafts: {e}")),
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
        match server.repo.delete_draft(&params.change_id, id).await {
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
    server.text(lines.join("\n"))
}

pub async fn publish_drafts<R: GerritRepository + Send + Sync + 'static>(
    server: &GerritServer<R>,
    params: PublishDraftsParams,
) -> CallToolResult {
    if let Some(r) = server.check_not_readonly("publish drafts") {
        return r;
    }
    let payload = PublishDraftsRequest { notify: None };
    match server
        .repo
        .publish_drafts(&params.change_id, &payload)
        .await
    {
        Ok(()) => server.text("All drafts published.".to_string()),
        Err(e) => server.error(format!("Failed to publish drafts: {e}")),
    }
}
