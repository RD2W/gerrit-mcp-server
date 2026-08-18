// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use super::*;

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
    pub set_labels_results: Mutex<Vec<Result<(), DomainError>>>,
    pub post_draft_results: Mutex<Vec<Result<String, DomainError>>>,
    pub delete_draft_results: Mutex<Vec<Result<(), DomainError>>>,
    pub publish_drafts_results: Mutex<Vec<Result<(), DomainError>>>,
    pub cherry_pick_results: Mutex<Vec<Result<CherryPickResult, DomainError>>>,
    pub get_related_results: Mutex<Vec<Result<Vec<RelatedChange>, DomainError>>>,
    pub submit_change_results: Mutex<Vec<Result<SubmitResult, DomainError>>>,
}

impl MockGerritRepository {
    #[must_use]
    pub fn make_change(number: u64, subject: &str) -> Change {
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
            topic: None,
            reviewers: None,
        }
    }

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

    pub fn push_set_labels_result(&self, result: Result<(), DomainError>) {
        self.set_labels_results.lock().unwrap().push(result);
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

    async fn set_labels(
        &self,
        _change_id: &str,
        _payload: &ReviewInput,
    ) -> Result<(), DomainError> {
        pop_result!(self.set_labels_results)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_query_changes_returns_pushed_result() {
        let mock = MockGerritRepository::default();
        let expected = MockGerritRepository::make_change(12345, "Test change");
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
            topic: None,
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
