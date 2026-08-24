// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

use std::collections::BTreeMap;
use std::time::Duration;

use gerrit_core::application::GerritService;
use gerrit_core::domain::*;
use gerrit_core::infrastructure::auth::AuthMode;
use gerrit_core::infrastructure::client::{GerritClient, GerritClientConfig};
use gerrit_core::infrastructure::tls::TlsConfig;
use gerrit_mcp::config::{AuthConfig, Config};
use gerrit_mcp::mcp::GerritServer;
use gerrit_mcp::mcp::tools::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ContentBlock;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_tls_config() -> TlsConfig {
    TlsConfig {
        verify_ssl: false,
        ..Default::default()
    }
}

fn extract_text(result: rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .find_map(|c| match c {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Config + Auth integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_config_from_toml_with_basic_auth() {
    let toml_str = r#"
[gerrit]
base_url = "https://gerrit.example.com"
[gerrit.auth]
mode = "basic"
username_env = "GERRIT_USERNAME"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gerrit.base_url, "https://gerrit.example.com");
    assert_eq!(config.gerrit.auth.mode, "basic");
    assert_eq!(
        config.gerrit.auth.username_env,
        Some("GERRIT_USERNAME".into())
    );
}

#[test]
fn test_config_from_toml_with_bearer_auth() {
    let toml_str = r#"
[gerrit]
base_url = "https://gerrit.example.com"
[gerrit.auth]
mode = "bearer"
token_env = "GERRIT_TOKEN"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gerrit.auth.mode, "bearer");
    assert_eq!(config.gerrit.auth.token_env, Some("GERRIT_TOKEN".into()));
}

#[test]
fn test_config_default_auth_mode_is_none() {
    let config = AuthConfig::default();
    assert_eq!(config.mode, "none");
    assert!(config.username.is_none());
    assert!(config.bearer_token.is_none());
    assert!(config.auth_token.is_none());
}

#[test]
fn test_config_default_env_vars_for_auth() {
    let config = AuthConfig::default();
    assert_eq!(config.auth_token_env, Some("GERRIT_AUTH_TOKEN".into()));
    assert_eq!(config.token_env, Some("GERRIT_TOKEN".into()));
}

#[test]
fn test_config_from_toml_with_full_gerrit() {
    let toml_str = r#"
[gerrit]
base_url = "https://gerrit.example.com"
timeout_secs = 60
verify_ssl = false

[gerrit.auth]
mode = "basic"
username_env = "GERRIT_USERNAME"
auth_token_env = "GERRIT_AUTH_TOKEN"

[transport]
mode = "http"
bind_addr = "127.0.0.1:3000"
allowed_hosts = ["localhost"]
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gerrit.timeout_secs, 60);
    assert!(!config.gerrit.verify_ssl);
    assert_eq!(config.transport.mode, "http");
    assert_eq!(config.transport.bind_addr, "127.0.0.1:3000");
    assert_eq!(config.transport.allowed_hosts, vec!["localhost"]);
}

#[test]
fn test_config_validation_base_url_empty() {
    let config = Config::default();
    let result = config.validate();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("base_url"));
}

// ---------------------------------------------------------------------------
// GerritClient + wiremock integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_client_bearer_auth_header_sent() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/changes/"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;

    let client = GerritClient::new(GerritClientConfig {
        base_url: server.uri(),
        auth: AuthMode::Bearer("test-token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let result = client.query_changes("status:open", None, &[]).await;

    match result {
        Ok(changes) => assert!(changes.is_empty()),
        Err(e) => panic!("expected Ok, got error: {e} — Bearer header may not have been sent"),
    }
}

#[tokio::test]
async fn test_client_bearer_auth_header_missing_rejected() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/changes/"))
        .and(header("Authorization", "Bearer correct-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;

    let client = GerritClient::new(GerritClientConfig {
        base_url: server.uri(),
        auth: AuthMode::Bearer("wrong-token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let result = client.query_changes("status:open", None, &[]).await;

    assert!(
        result.is_err(),
        "expected error because Bearer token does not match mock expectation"
    );
}

#[tokio::test]
async fn test_client_get_change_detail_parsing() {
    let server = MockServer::start().await;

    let change_detail_json = r#"{
        "id": "Iabc123",
        "_number": 4242,
        "subject": "Fix memory leak",
        "status": "NEW",
        "project": "my-project",
        "branch": "main",
        "owner": {
            "_account_id": 9001,
            "name": "Alice Developer",
            "email": "alice@example.com"
        },
        "updated": "2026-07-01 12:00:00",
        "currentRevision": "rev-hash-abc",
        "currentRevisionNumber": 3,
        "revisions": {
            "rev-hash-abc": {
                "_number": 3,
                "commit": {
                    "message": "Fix memory leak in allocator\n\nBug: 12345"
                }
            }
        },
        "labels": {},
        "messages": [
            {
                "author": {
                    "_account_id": 9002,
                    "name": "Bob Reviewer",
                    "email": "bob@example.com"
                },
                "date": "2026-07-01 13:00:00",
                "message": "Looks good to me",
                "_revision_number": 3
            }
        ]
    }"#;

    Mock::given(method("GET"))
        .and(path("/changes/Iabc123/detail"))
        .respond_with(ResponseTemplate::new(200).set_body_string(change_detail_json))
        .mount(&server)
        .await;

    let client = GerritClient::new(GerritClientConfig {
        base_url: server.uri(),
        auth: AuthMode::Bearer("test-token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let detail = client.get_change_detail("Iabc123", &[]).await.unwrap();

    assert_eq!(detail._number, 4242);
    assert_eq!(detail.subject, "Fix memory leak");
    assert_eq!(detail.status, "NEW");
    assert_eq!(detail.project, "my-project");
    assert_eq!(detail.branch, "main");
    assert_eq!(detail.owner._account_id, 9001);
    assert_eq!(detail.owner.name, Some("Alice Developer".into()));
    assert_eq!(detail.owner.email, Some("alice@example.com".into()));
    assert_eq!(detail.updated, "2026-07-01 12:00:00");
    assert_eq!(detail.current_revision, Some("rev-hash-abc".into()));
    assert_eq!(detail.current_revision_number, Some(3));
    assert_eq!(detail.revisions.len(), 1);
    assert_eq!(detail.messages.len(), 1);
    assert_eq!(
        detail.messages[0].author.as_ref().unwrap().email,
        Some("bob@example.com".into())
    );
}

#[tokio::test]
async fn test_client_xssi_stripping_in_get_change_detail() {
    let server = MockServer::start().await;

    let body = ")]}'\n{\"id\":\"xssitest\",\"_number\":1,\"subject\":\"s\",\"status\":\"NEW\",\"project\":\"p\",\"branch\":\"b\",\"owner\":{\"_account_id\":1},\"updated\":\"now\"}";

    Mock::given(method("GET"))
        .and(path("/changes/xssitest/detail"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let client = GerritClient::new(GerritClientConfig {
        base_url: server.uri(),
        auth: AuthMode::Bearer("test-token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let detail = client.get_change_detail("xssitest", &[]).await.unwrap();
    assert_eq!(detail._number, 1);
    assert_eq!(detail.subject, "s");
}

#[tokio::test]
async fn test_client_list_files_parsing() {
    let server = MockServer::start().await;

    let files_json = r#"{
        "src/main.rs": {"status": "M", "linesInserted": 10, "linesDeleted": 2},
        "src/lib.rs": {"status": "A", "linesInserted": 50, "linesDeleted": 0},
        "/COMMIT_MSG": {"status": "M", "linesInserted": 5, "linesDeleted": 0}
    }"#;

    Mock::given(method("GET"))
        .and(path("/changes/123/revisions/current/files/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(files_json))
        .mount(&server)
        .await;

    let client = GerritClient::new(GerritClientConfig {
        base_url: server.uri(),
        auth: AuthMode::Bearer("test-token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let files = client.list_files("123").await.unwrap();

    assert_eq!(files.len(), 3);
    assert_eq!(files["src/main.rs"].status, Some("M".into()));
    assert_eq!(files["src/main.rs"].lines_inserted, 10);
    assert_eq!(files["src/main.rs"].lines_deleted, 2);
    assert_eq!(files["src/lib.rs"].status, Some("A".into()));
    assert_eq!(files["src/lib.rs"].lines_inserted, 50);
}

#[tokio::test]
async fn test_multi_instance_query_changes() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let change_a = r#"{"id":"a~1","_number":100,"subject":"From A","status":"NEW","project":"p","branch":"b","owner":{"_account_id":1},"updated":"2025-01-01 00:00:00"}"#;
    let change_b = r#"{"id":"b~2","_number":200,"subject":"From B","status":"NEW","project":"p","branch":"b","owner":{"_account_id":1},"updated":"2025-01-01 00:00:00"}"#;

    Mock::given(method("GET"))
        .and(path("/changes/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("[{change_a}]")))
        .mount(&server_a)
        .await;

    Mock::given(method("GET"))
        .and(path("/changes/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("[{change_b}]")))
        .mount(&server_b)
        .await;

    let default_client = GerritClient::new(GerritClientConfig {
        base_url: server_a.uri(),
        auth: AuthMode::Bearer("token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let factory_config = GerritClientConfig {
        base_url: String::new(),
        auth: AuthMode::Bearer("token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    };

    let service = GerritService::new(default_client);
    let server = GerritServer::new(service).with_client_factory(factory_config);

    let params_a = QueryChangesParams {
        query: "status:open".into(),
        gerrit_base_url: Some(server_a.uri()),
        limit: None,
        options: None,
    };
    let result = server.query_changes(Parameters(params_a)).await;
    let text = extract_text(result);
    assert!(text.contains("From A"), "expected 'From A' in {text}");

    let params_b = QueryChangesParams {
        query: "status:open".into(),
        gerrit_base_url: Some(server_b.uri()),
        limit: None,
        options: None,
    };
    let result = server.query_changes(Parameters(params_b)).await;
    let text = extract_text(result);
    assert!(text.contains("From B"), "expected 'From B' in {text}");
}

#[tokio::test]
async fn test_create_change_honors_gerrit_base_url_override() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let change_json = r#"{"id":"proj~main~777","_number":777,"subject":"Created via override","status":"NEW","project":"proj","branch":"main","owner":{"_account_id":1},"updated":"2025-01-01 00:00:00"}"#;

    Mock::given(method("POST"))
        .and(path("/changes/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(change_json))
        .mount(&server_b)
        .await;

    let default_client = GerritClient::new(GerritClientConfig {
        base_url: server_a.uri(),
        auth: AuthMode::Bearer("token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    })
    .unwrap();

    let factory_config = GerritClientConfig {
        base_url: String::new(),
        auth: AuthMode::Bearer("token".into()),
        timeout: Duration::from_secs(5),
        tls: test_tls_config(),
        disable_url_normalization: true,
    };

    let service = GerritService::new(default_client);
    let server = GerritServer::new(service).with_client_factory(factory_config);

    let params = CreateChangeParams {
        project: "proj".into(),
        subject: "Override subject".into(),
        branch: "main".into(),
        topic: None,
        status: None,
        gerrit_base_url: Some(server_b.uri()),
    };
    let result = server.create_change(Parameters(params)).await;
    let text = extract_text(result);
    assert!(
        text.contains("Created via override"),
        "expected override-host response in {text}"
    );
}

// ---------------------------------------------------------------------------
// GerritServer full pipeline tests (MockGerritRepository)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_query_changes_full_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_query_changes_result(Ok(vec![
        MockGerritRepository::make_change(100, "First change"),
        MockGerritRepository::make_change(200, "Second change"),
    ]));
    let server = GerritServer::new(mock);

    let params = QueryChangesParams {
        query: "status:open".into(),
        gerrit_base_url: None,
        limit: Some(5),
        options: None,
    };
    let result = server.query_changes(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("100_"));
    assert!(text.contains("First change"));
    assert!(text.contains("200_"));
    assert!(text.contains("Second change"));
}

#[tokio::test]
async fn test_query_changes_empty_result() {
    let mock = MockGerritRepository::default();
    mock.push_query_changes_result(Ok(vec![]));
    let server = GerritServer::new(mock);

    let params = QueryChangesParams {
        query: "status:open".into(),
        gerrit_base_url: None,
        limit: None,
        options: None,
    };
    let result = server.query_changes(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("No changes found"));
}

#[tokio::test]
async fn test_query_changes_error_propagation() {
    let mock = MockGerritRepository::default();
    mock.push_query_changes_result(Err(DomainError::EmptyQuery));
    let server = GerritServer::new(mock);

    let params = QueryChangesParams {
        query: "".into(),
        gerrit_base_url: None,
        limit: None,
        options: None,
    };
    let result = server.query_changes(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Failed to query changes"));
}

#[tokio::test]
async fn test_get_change_detail_pipeline() {
    let mock = MockGerritRepository::default();
    let mut revisions = BTreeMap::new();
    revisions.insert(
        "rev1".into(),
        RevisionInfo {
            _number: 1,
            commit: Some(CommitWithMessage {
                message: "Fix things\n\nBug: 42".into(),
            }),
        },
    );
    mock.push_get_change_detail_result(Ok(ChangeDetail {
        id: "project~123".into(),
        _number: 123,
        subject: "Important fix".into(),
        status: "NEW".into(),
        project: "p".into(),
        branch: "main".into(),
        owner: AccountInfo {
            _account_id: 1,
            name: Some("Dev".into()),
            email: Some("dev@example.com".into()),
        },
        updated: "now".into(),
        current_revision: Some("rev1".into()),
        current_revision_number: Some(1),
        revisions,
        labels: BTreeMap::new(),
        reviewers: None,
        messages: vec![],
        topic: None,
    }));
    let server = GerritServer::new(mock);

    let params = GetChangeDetailsParams {
        change_id: "project~123".into(),
        gerrit_base_url: None,
        options: None,
    };
    let result = server.get_change_details(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Important fix"));
    assert!(text.contains("Dev <dev@example.com>"));
    assert!(text.contains("NEW"));
    assert!(text.contains("Bugs: 42"));
}

#[tokio::test]
async fn test_get_commit_message_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_get_commit_message_result(Ok(CommitMessage {
        full_message: "Fix stuff\n\nDetails here\n\nChange-Id: Iabc123".into(),
    }));
    let server = GerritServer::new(mock);

    let params = GetCommitMessageParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.get_commit_message(Parameters(params)).await;
    let text = extract_text(result);

    assert_eq!(
        text, "Fix stuff\n\nDetails here\n\nChange-Id: Iabc123",
        "commit message must be returned verbatim"
    );
}

#[tokio::test]
async fn test_list_files_pipeline() {
    let mock = MockGerritRepository::default();
    let mut files = BTreeMap::new();
    files.insert(
        "src/main.rs".into(),
        FileInfo {
            status: Some("M".into()),
            lines_inserted: 10,
            lines_deleted: 2,
        },
    );
    files.insert(
        "src/lib.rs".into(),
        FileInfo {
            status: Some("A".into()),
            lines_inserted: 42,
            lines_deleted: 0,
        },
    );
    mock.push_list_files_result(Ok(files));
    let server = GerritServer::new(mock);

    let params = ListChangeFilesParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.list_change_files(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("[M] src/main.rs (+10, -2)"));
    assert!(text.contains("[A] src/lib.rs (+42, -0)"));
    assert!(!text.contains("/COMMIT_MSG"));
}

#[tokio::test]
async fn test_list_comments_pipeline() {
    let mock = MockGerritRepository::default();
    let mut comments = BTreeMap::new();
    comments.insert(
        "src/main.rs".into(),
        vec![Comment {
            id: "c1".into(),
            line: Some(42),
            message: "Please fix this".into(),
            author: Some(AccountInfo {
                _account_id: 2,
                name: None,
                email: Some("reviewer@test.com".into()),
            }),
            updated: "now".into(),
            unresolved: Some(true),
        }],
    );
    mock.push_list_comments_result(Ok(comments));
    let server = GerritServer::new(mock);

    let params = ListChangeCommentsParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.list_change_comments(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("src/main.rs"));
    assert!(text.contains("L42"));
    assert!(text.contains("[unresolved]"));
    assert!(text.contains("Please fix this"));
    assert!(text.contains("reviewer@test.com"));
}

#[tokio::test]
async fn test_list_comments_empty() {
    let mock = MockGerritRepository::default();
    mock.push_list_comments_result(Ok(BTreeMap::new()));
    let server = GerritServer::new(mock);

    let params = ListChangeCommentsParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.list_change_comments(Parameters(params)).await;
    let text = extract_text(result);

    assert_eq!(text, "No comments.");
}

#[tokio::test]
async fn test_get_bugs_from_cl_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_get_commit_result(Ok(CommitInfo {
        message: "Fix critical issue\n\nBug: 12345\nFixes: 67890\nAlso b/99999".into(),
    }));
    let server = GerritServer::new(mock);

    let params = GetBugsFromClParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.get_bugs_from_cl(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("12345"));
    assert!(text.contains("67890"));
    assert!(text.contains("99999"));
}

#[tokio::test]
async fn test_get_bugs_from_cl_no_bugs() {
    let mock = MockGerritRepository::default();
    mock.push_get_commit_result(Ok(CommitInfo {
        message: "No bug references".into(),
    }));
    let server = GerritServer::new(mock);

    let params = GetBugsFromClParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.get_bugs_from_cl(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("No bugs found"));
}

#[tokio::test]
async fn test_suggest_reviewers_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_suggest_reviewers_result(Ok(vec![
        SuggestedReviewer {
            account: Some(AccountInfo {
                _account_id: 10,
                name: Some("Expert".into()),
                email: Some("expert@test.com".into()),
            }),
            group: None,
        },
        SuggestedReviewer {
            account: None,
            group: Some(GroupInfo {
                name: "core-team".into(),
            }),
        },
    ]));
    let server = GerritServer::new(mock);

    let params = SuggestReviewersParams {
        change_id: "123".into(),
        query: "exp".into(),
        limit: Some(10),
        exclude_groups: None,
        reviewer_state: None,
        gerrit_base_url: None,
    };
    let result = server.suggest_reviewers(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Expert <expert@test.com>"));
    assert!(text.contains("core-team"));
}

#[tokio::test]
async fn test_add_reviewer_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_add_reviewer_result(Ok(AddReviewerResult {
        error: None,
        reviewers: vec![ReviewerInfo {
            _account_id: 5,
            email: Some("new-reviewer@test.com".into()),
        }],
    }));
    let server = GerritServer::new(mock);

    let params = AddReviewerParams {
        change_id: "123".into(),
        reviewer: "new-reviewer@test.com".into(),
        gerrit_base_url: None,
        state: None,
        confirmed: None,
    };
    let result = server.add_reviewer(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("new-reviewer@test.com"));
    assert!(text.contains("REVIEWER"));
}

#[tokio::test]
async fn test_add_reviewer_error() {
    let mock = MockGerritRepository::default();
    mock.push_add_reviewer_result(Ok(AddReviewerResult {
        error: Some("Account not found".into()),
        reviewers: vec![],
    }));
    let server = GerritServer::new(mock);

    let params = AddReviewerParams {
        change_id: "123".into(),
        reviewer: "unknown@test.com".into(),
        gerrit_base_url: None,
        state: None,
        confirmed: None,
    };
    let result = server.add_reviewer(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Account not found"));
}

#[tokio::test]
async fn test_create_change_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_create_change_result(Ok(MockGerritRepository::make_change(500, "New feature")));
    let server = GerritServer::new(mock);

    let params = CreateChangeParams {
        project: "my-proj".into(),
        subject: "New feature".into(),
        branch: "main".into(),
        topic: None,
        status: None,
        gerrit_base_url: None,
    };
    let result = server.create_change(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Created change"));
    assert!(text.contains("500"));
    assert!(text.contains("New feature"));
}

#[tokio::test]
async fn test_abandon_change_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_abandon_change_result(Ok(MockGerritRepository::make_change(
        600,
        "Abandoned feature",
    )));
    let server = GerritServer::new(mock);

    let params = AbandonChangeParams {
        change_id: "600".into(),
        message: Some("No longer needed".into()),
        gerrit_base_url: None,
    };
    let result = server.abandon_change(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("600"));
    assert!(text.contains("Abandoned feature"));
}

#[tokio::test]
async fn test_revert_change_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_revert_change_result(Ok(MockGerritRepository::make_change(
        700,
        "Revert \"bad change\"",
    )));
    let server = GerritServer::new(mock);

    let params = RevertChangeParams {
        change_id: "700".into(),
        message: None,
        gerrit_base_url: None,
    };
    let result = server.revert_change(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Revert created"));
    assert!(text.contains("700"));
}

#[tokio::test]
async fn test_revert_submission_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_revert_submission_result(Ok(vec![
        MockGerritRepository::make_change(800, "Revert 1"),
        MockGerritRepository::make_change(801, "Revert 2"),
    ]));
    let server = GerritServer::new(mock);

    let params = RevertSubmissionParams {
        change_id: "800".into(),
        message: None,
        gerrit_base_url: None,
    };
    let result = server.revert_submission(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Revert changes created"));
    assert!(text.contains("800"));
    assert!(text.contains("801"));
}

#[tokio::test]
async fn test_cherry_pick_change_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_cherry_pick_result(Ok(CherryPickResult {
        id: "new~999".into(),
        _number: 999,
        subject: "Cherry-picked fix".into(),
    }));
    let server = GerritServer::new(mock);

    let params = CherryPickChangeParams {
        change_id: "12345".into(),
        destination: "release".into(),
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
async fn test_submit_change_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_submit_change_result(Ok(SubmitResult {
        id: "test~42".into(),
        _number: 42,
        subject: "Merged change".into(),
        status: "MERGED".into(),
    }));
    let server = GerritServer::new(mock);

    let params = SubmitChangeParams {
        change_id: "42".into(),
        wait_for_merge: None,
        gerrit_base_url: None,
    };
    let result = server.submit_change(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Successfully submitted"));
    assert!(text.contains("MERGED"));
}

#[tokio::test]
async fn test_changes_submitted_together_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_changes_submitted_together_result(Ok(SubmittedTogether {
        changes: vec![
            MockGerritRepository::make_change(10, "Related 1"),
            MockGerritRepository::make_change(11, "Related 2"),
        ],
        non_visible_changes: 3,
    }));
    let server = GerritServer::new(mock);

    let params = ChangesSubmittedTogetherParams {
        change_id: "10".into(),
        gerrit_base_url: None,
        options: None,
    };
    let result = server.changes_submitted_together(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("10_"));
    assert!(text.contains("Related 1"));
    assert!(text.contains("11_"));
    assert!(text.contains("Related 2"));
    assert!(text.contains("3 changes not visible"));
}

#[tokio::test]
async fn test_post_review_comment_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_post_review_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = PostReviewCommentParams {
        change_id: "123".into(),
        file_path: "src/main.rs".into(),
        line_number: 42,
        message: "Looks good".into(),
        unresolved: None,
        gerrit_base_url: None,
        labels: None,
    };
    let result = server.post_review_comment(Parameters(params)).await;
    let text = extract_text(result);

    assert_eq!(text, "Review comment posted.");
}

#[tokio::test]
async fn test_post_draft_comment_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_post_draft_result(Ok("draft_42".into()));
    let server = GerritServer::new(mock);

    let params = PostDraftCommentParams {
        change_id: "123".into(),
        file_path: "src/main.rs".into(),
        line_number: 10,
        message: "Needs work".into(),
        unresolved: None,
        gerrit_base_url: None,
        start_line: None,
        start_character: None,
        end_line: None,
        end_character: None,
        suggestion: None,
        in_reply_to: None,
    };
    let result = server.post_draft_comment(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Draft comment posted"));
    assert!(text.contains("draft_42"));
}

#[tokio::test]
async fn test_set_topic_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_set_topic_result(Ok(Some("my-topic".into())));
    let server = GerritServer::new(mock);

    let params = SetTopicParams {
        change_id: "123".into(),
        topic: "my-topic".into(),
        gerrit_base_url: None,
    };
    let result = server.set_topic(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Topic set to 'my-topic'"));
}

#[tokio::test]
async fn test_set_topic_delete() {
    let mock = MockGerritRepository::default();
    mock.push_set_topic_result(Ok(None));
    let server = GerritServer::new(mock);

    let params = SetTopicParams {
        change_id: "123".into(),
        topic: "".into(),
        gerrit_base_url: None,
    };
    let result = server.set_topic(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Topic deleted"));
}

#[tokio::test]
async fn test_set_ready_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_set_ready_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = SetReadyParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.set_ready_for_review(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("marked as ready for review"));
}

#[tokio::test]
async fn test_set_wip_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_set_wip_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = SetWipParams {
        change_id: "123".into(),
        message: Some("Still working on it".into()),
        gerrit_base_url: None,
    };
    let result = server.set_work_in_progress(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("work-in-progress"));
}

#[tokio::test]
async fn test_publish_drafts_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_publish_drafts_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = PublishDraftsParams {
        change_id: "123".into(),
        message: Some("Addressed all comments".into()),
        labels: Some(BTreeMap::from([("Code-Review".into(), 1)])),
        gerrit_base_url: None,
    };
    let result = server.publish_drafts(Parameters(params)).await;
    let text = extract_text(result);

    assert_eq!(text, "All drafts published.");
    // The handler must always send drafts explicitly — Gerrit's "Set Review"
    // endpoint defaults to KEEP, which returns success without publishing.
    let captured = server
        .repo
        .last_publish_drafts_payload
        .read()
        .unwrap()
        .clone()
        .expect("publish_drafts must record the payload");
    assert_eq!(captured.drafts, DraftHandling::PublishAllRevisions);
    assert_eq!(captured.message.as_deref(), Some("Addressed all comments"));
    assert_eq!(
        captured.labels,
        Some(BTreeMap::from([("Code-Review".into(), 1)]))
    );
}

#[tokio::test]
async fn test_delete_draft_comment_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_delete_draft_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = DeleteDraftCommentParams {
        change_id: "123".into(),
        draft_id: "draft_1".into(),
        gerrit_base_url: None,
    };
    let result = server.delete_draft_comment(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("deleted"));
}

#[tokio::test]
async fn test_delete_draft_comments_pipeline() {
    let mut drafts = BTreeMap::new();
    drafts.insert(
        "src/main.rs".into(),
        vec![
            DraftComment {
                id: "d1".into(),
                path: "src/main.rs".into(),
                line: Some(1),
                message: "fixme".into(),
            },
            DraftComment {
                id: "d2".into(),
                path: "src/main.rs".into(),
                line: Some(2),
                message: "todo".into(),
            },
        ],
    );

    let mock = MockGerritRepository::default();
    mock.push_list_drafts_result(Ok(drafts));
    mock.push_delete_draft_result(Ok(()));
    mock.push_delete_draft_result(Ok(()));
    let server = GerritServer::new(mock);

    let params = DeleteDraftCommentsParams {
        change_id: "123".into(),
        gerrit_base_url: None,
    };
    let result = server.delete_draft_comments(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Deleted 2 of 2"));
}

#[tokio::test]
async fn test_cherry_pick_chain_pipeline() {
    let mock = MockGerritRepository::default();
    mock.push_get_related_result(Ok(vec![
        RelatedChange {
            _change_number: 1,
            _revision_number: 1,
        },
        RelatedChange {
            _change_number: 2,
            _revision_number: 1,
        },
    ]));
    mock.push_cherry_pick_result(Ok(CherryPickResult {
        id: "new~100".into(),
        _number: 100,
        subject: "Cp1".into(),
    }));
    let mut revs = BTreeMap::new();
    revs.insert(
        "rev1".into(),
        RevisionInfo {
            _number: 1,
            commit: Some(CommitWithMessage {
                message: "hash123".into(),
            }),
        },
    );
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
        revisions: revs,
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
        current_revision: Some("rev101".into()),
        current_revision_number: Some(1),
        revisions: BTreeMap::new(),
        labels: BTreeMap::new(),
        reviewers: None,
        messages: vec![],
        topic: None,
    }));

    let server = GerritServer::new(mock);

    let params = CherryPickChainParams {
        change_id: "2".into(),
        destination: "main".into(),
        revision_id: None,
        keep_reviewers: None,
        allow_conflicts: None,
        allow_empty: None,
        gerrit_base_url: None,
    };
    let result = server.cherry_pick_chain(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Successfully cherry-picked chain of 2"));
    assert!(text.contains("100"));
    assert!(text.contains("101"));
}

#[tokio::test]
async fn test_cherry_pick_chain_partial_failure() {
    let mock = MockGerritRepository::default();
    mock.push_get_related_result(Ok(vec![
        RelatedChange {
            _change_number: 1,
            _revision_number: 1,
        },
        RelatedChange {
            _change_number: 2,
            _revision_number: 1,
        },
    ]));
    mock.push_cherry_pick_result(Err(DomainError::HttpStatus {
        status: 409,
        body: "Conflict".into(),
    }));
    let mut revs = BTreeMap::new();
    revs.insert(
        "rev1".into(),
        RevisionInfo {
            _number: 1,
            commit: Some(CommitWithMessage {
                message: "hash456".into(),
            }),
        },
    );
    mock.push_get_change_detail_result(Ok(ChangeDetail {
        id: "new~200".into(),
        _number: 200,
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
        revisions: revs,
        labels: BTreeMap::new(),
        reviewers: None,
        messages: vec![],
        topic: None,
    }));
    mock.push_cherry_pick_result(Ok(CherryPickResult {
        id: "new~200".into(),
        _number: 200,
        subject: "Cp1".into(),
    }));

    let server = GerritServer::new(mock);

    let params = CherryPickChainParams {
        change_id: "2".into(),
        destination: "main".into(),
        revision_id: None,
        keep_reviewers: None,
        allow_conflicts: None,
        allow_empty: None,
        gerrit_base_url: None,
    };
    let result = server.cherry_pick_chain(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Some changes were cherry-picked successfully"));
    assert!(text.contains("200"));
    assert!(text.contains("Partial failure"));
}

#[tokio::test]
async fn test_invalid_add_reviewer_state() {
    let mock = MockGerritRepository::default();
    let server = GerritServer::new(mock);

    let params = AddReviewerParams {
        change_id: "123".into(),
        reviewer: "someone@test.com".into(),
        gerrit_base_url: None,
        state: Some("INVALID".into()),
        confirmed: None,
    };
    let result = server.add_reviewer(Parameters(params)).await;
    let text = extract_text(result);

    assert!(text.contains("Invalid state"));
}
