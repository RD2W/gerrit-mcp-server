// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Application service layer — wraps a repository with optional caching and rate-limiting.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::*;
use crate::infrastructure::cache::MemoryCache;
use crate::infrastructure::rate_limit::TokenBucket;

pub struct GerritService<R: GerritRepository> {
    repo: Arc<R>,
    cache: Option<MemoryCache<String, String>>,
    rate_limiter: Option<TokenBucket>,
}

impl<R: GerritRepository> Clone for GerritService<R> {
    fn clone(&self) -> Self {
        Self {
            repo: self.repo.clone(),
            cache: self.cache.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

impl<R: GerritRepository> GerritService<R> {
    pub fn new(repo: R) -> Self {
        Self {
            repo: Arc::new(repo),
            cache: None,
            rate_limiter: None,
        }
    }

    pub fn with_cache(mut self, ttl: Duration, max_entries: usize) -> Self {
        self.cache = Some(MemoryCache::new(ttl, max_entries));
        self
    }

    pub fn with_rate_limit(mut self, requests_per_second: u32, burst: u32) -> Self {
        self.rate_limiter = Some(TokenBucket::new(requests_per_second, burst));
        self
    }

    async fn acquire_rate_limit(&self) -> Result<(), DomainError> {
        if let Some(ref rl) = self.rate_limiter {
            rl.acquire().await?;
        }
        Ok(())
    }
}

/// Cache key builder for common Gerrit API calls.
fn cache_key(prefix: &str, args: &str) -> String {
    format!("{prefix}|{args}")
}

#[async_trait::async_trait]
impl<R: GerritRepository> GerritRepository for GerritService<R> {
    async fn query_changes(
        &self,
        query: &str,
        limit: Option<u32>,
        options: &[String],
    ) -> Result<Vec<Change>, DomainError> {
        let key = cache_key("qc", &format!("{query}:{limit:?}:{options:?}"));
        if let Some(ref cache) = self.cache
            && let Some(cached) = cache.get(&key)
        {
            return serde_json::from_str(&cached).map_err(|e| DomainError::Decode(e.to_string()));
        }
        self.acquire_rate_limit().await?;
        let result = self.repo.query_changes(query, limit, options).await?;
        if let Some(ref cache) = self.cache
            && let Ok(json) = serde_json::to_string(&result)
        {
            cache.insert(key, json);
        }
        Ok(result)
    }

    async fn get_change_detail(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<ChangeDetail, DomainError> {
        let key = cache_key("gcd", &format!("{change_id}:{options:?}"));
        if let Some(ref cache) = self.cache
            && let Some(cached) = cache.get(&key)
        {
            return serde_json::from_str(&cached).map_err(|e| DomainError::Decode(e.to_string()));
        }
        self.acquire_rate_limit().await?;
        let result = self.repo.get_change_detail(change_id, options).await?;
        if let Some(ref cache) = self.cache
            && let Ok(json) = serde_json::to_string(&result)
        {
            cache.insert(key, json);
        }
        Ok(result)
    }

    async fn get_commit_message(&self, change_id: &str) -> Result<CommitMessage, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.get_commit_message(change_id).await
    }

    async fn list_files(&self, change_id: &str) -> Result<BTreeMap<String, FileInfo>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.list_files(change_id).await
    }

    async fn get_diff(&self, change_id: &str, file_path: &str) -> Result<String, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.get_diff(change_id, file_path).await
    }

    async fn list_comments(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<Comment>>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.list_comments(change_id).await
    }

    async fn list_drafts(
        &self,
        change_id: &str,
    ) -> Result<BTreeMap<String, Vec<DraftComment>>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.list_drafts(change_id).await
    }

    async fn get_commit(&self, change_id: &str) -> Result<CommitInfo, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.get_commit(change_id).await
    }

    async fn suggest_reviewers(
        &self,
        change_id: &str,
        query: &str,
        limit: Option<u32>,
        exclude_groups: bool,
        reviewer_state: Option<&str>,
    ) -> Result<Vec<SuggestedReviewer>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo
            .suggest_reviewers(change_id, query, limit, exclude_groups, reviewer_state)
            .await
    }

    async fn changes_submitted_together(
        &self,
        change_id: &str,
        options: &[String],
    ) -> Result<SubmittedTogether, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo
            .changes_submitted_together(change_id, options)
            .await
    }

    async fn create_change(&self, payload: &CreateChangeRequest) -> Result<Change, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.create_change(payload).await
    }

    async fn add_reviewer(
        &self,
        change_id: &str,
        payload: &AddReviewerRequest,
    ) -> Result<AddReviewerResult, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.add_reviewer(change_id, payload).await
    }

    async fn set_ready(&self, change_id: &str) -> Result<(), DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.set_ready(change_id).await
    }

    async fn set_wip(&self, change_id: &str, payload: &WipRequest) -> Result<(), DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.set_wip(change_id, payload).await
    }

    async fn set_topic(
        &self,
        change_id: &str,
        payload: &TopicRequest,
    ) -> Result<Option<String>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.set_topic(change_id, payload).await
    }

    async fn abandon_change(
        &self,
        change_id: &str,
        payload: &AbandonRequest,
    ) -> Result<Change, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.abandon_change(change_id, payload).await
    }

    async fn revert_change(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Change, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.revert_change(change_id, message).await
    }

    async fn revert_submission(
        &self,
        change_id: &str,
        message: Option<&str>,
    ) -> Result<Vec<Change>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.revert_submission(change_id, message).await
    }

    async fn post_review(
        &self,
        change_id: &str,
        payload: &CommentBatchInput,
    ) -> Result<(), DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.post_review(change_id, payload).await
    }

    async fn post_draft(
        &self,
        change_id: &str,
        payload: &DraftInput,
    ) -> Result<String, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.post_draft(change_id, payload).await
    }

    async fn delete_draft(&self, change_id: &str, draft_id: &str) -> Result<(), DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.delete_draft(change_id, draft_id).await
    }

    async fn publish_drafts(
        &self,
        change_id: &str,
        payload: &PublishDraftsRequest,
    ) -> Result<(), DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.publish_drafts(change_id, payload).await
    }

    async fn cherry_pick(
        &self,
        change_id: &str,
        revision: &str,
        payload: &CherryPickRequest,
    ) -> Result<CherryPickResult, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.cherry_pick(change_id, revision, payload).await
    }

    async fn get_related(
        &self,
        change_id: &str,
        revision: &str,
    ) -> Result<Vec<RelatedChange>, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.get_related(change_id, revision).await
    }

    async fn submit_change(
        &self,
        change_id: &str,
        payload: &SubmitRequest,
    ) -> Result<SubmitResult, DomainError> {
        self.acquire_rate_limit().await?;
        self.repo.submit_change(change_id, payload).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_change(num: u64) -> Change {
        Change {
            id: format!("proj~master~I{num}"),
            _number: num,
            subject: format!("Change {num}"),
            status: "NEW".into(),
            project: "proj".into(),
            branch: "master".into(),
            owner: AccountInfo {
                _account_id: 1,
                name: None,
                email: None,
            },
            updated: "2025-01-01".into(),
            work_in_progress: false,
        }
    }

    fn test_detail(num: u64) -> ChangeDetail {
        ChangeDetail {
            id: format!("proj~master~I{num}"),
            _number: num,
            subject: format!("Detail {num}"),
            status: "NEW".into(),
            project: "proj".into(),
            branch: "master".into(),
            owner: AccountInfo {
                _account_id: 1,
                name: None,
                email: None,
            },
            updated: "2025-01-01".into(),
            current_revision: None,
            current_revision_number: None,
            revisions: BTreeMap::new(),
            labels: BTreeMap::new(),
            reviewers: None,
            messages: vec![],
        }
    }

    #[tokio::test]
    async fn delegates_to_repo_without_cache() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![test_change(1)]));
        let svc = GerritService::new(mock);
        let result = svc.query_changes("test", None, &[]).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]._number, 1);
    }

    #[tokio::test]
    async fn cache_hit_avoids_repo_call() {
        let mock = Arc::new(MockGerritRepository::default());
        mock.push_query_changes_result(Ok(vec![test_change(42)]));

        let svc = GerritService {
            repo: mock.clone(),
            cache: Some(MemoryCache::new(Duration::from_secs(60), 100)),
            rate_limiter: None,
        };

        let r1 = svc.query_changes("q", None, &[]).await.unwrap();
        assert_eq!(r1[0]._number, 42);
        assert_eq!(mock.query_changes_call_count(), 1);

        let r2 = svc.query_changes("q", None, &[]).await.unwrap();
        assert_eq!(r2[0]._number, 42);
        assert_eq!(
            mock.query_changes_call_count(),
            1,
            "second call should hit cache"
        );
    }

    #[tokio::test]
    async fn different_query_misses_cache() {
        let mock = Arc::new(MockGerritRepository::default());
        mock.push_query_changes_result(Ok(vec![test_change(2)]));
        mock.push_query_changes_result(Ok(vec![test_change(1)]));

        let svc = GerritService {
            repo: mock.clone(),
            cache: Some(MemoryCache::new(Duration::from_secs(60), 100)),
            rate_limiter: None,
        };

        let r1 = svc.query_changes("q1", None, &[]).await.unwrap();
        assert_eq!(r1[0]._number, 1);
        assert_eq!(mock.query_changes_call_count(), 1);

        let r2 = svc.query_changes("q2", None, &[]).await.unwrap();
        assert_eq!(r2[0]._number, 2);
        assert_eq!(mock.query_changes_call_count(), 2);
    }

    #[tokio::test]
    async fn cache_detail_hit_avoids_repo() {
        let mock = Arc::new(MockGerritRepository::default());
        mock.push_get_change_detail_result(Ok(test_detail(99)));

        let svc = GerritService {
            repo: mock.clone(),
            cache: Some(MemoryCache::new(Duration::from_secs(60), 100)),
            rate_limiter: None,
        };

        let d1 = svc.get_change_detail("99", &[]).await.unwrap();
        assert_eq!(d1._number, 99);

        let d2 = svc.get_change_detail("99", &[]).await.unwrap();
        assert_eq!(d2._number, 99);
    }

    #[tokio::test]
    async fn rate_limiter_allows_call() {
        let mock = MockGerritRepository::default();
        mock.push_query_changes_result(Ok(vec![test_change(1)]));
        let svc = GerritService::new(mock).with_rate_limit(1000, 500);
        let result = svc.query_changes("test", None, &[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn write_operations_not_cached() {
        let mock = MockGerritRepository::default();
        mock.push_create_change_result(Ok(test_change(77)));
        let svc = GerritService::new(mock)
            .with_cache(Duration::from_secs(60), 100)
            .with_rate_limit(100, 200);

        let payload = CreateChangeRequest {
            project: "p".into(),
            branch: "b".into(),
            subject: "s".into(),
            topic: None,
            status: None,
            is_private: None,
            work_in_progress: None,
            base_change: None,
            new_branch: None,
        };
        let result = svc.create_change(&payload).await.unwrap();
        assert_eq!(result._number, 77);
    }

    #[tokio::test]
    async fn error_from_repo_not_cached() {
        let mock = Arc::new(MockGerritRepository::default());
        mock.push_query_changes_result(Err(DomainError::HttpStatus {
            status: 500,
            body: "err".into(),
        }));

        let svc = GerritService {
            repo: mock.clone(),
            cache: Some(MemoryCache::new(Duration::from_secs(60), 100)),
            rate_limiter: None,
        };

        let err = svc.query_changes("q", None, &[]).await.unwrap_err();
        assert!(matches!(err, DomainError::HttpStatus { .. }));

        mock.push_query_changes_result(Ok(vec![test_change(1)]));
        let r = svc.query_changes("q", None, &[]).await.unwrap();
        assert_eq!(r[0]._number, 1);
        assert_eq!(
            mock.query_changes_call_count(),
            2,
            "error result should NOT be cached"
        );
    }
}
