// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Streamable HTTP transport via axum + rmcp StreamableHttpService.

use std::sync::Arc;

use axum::{
    Router,
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use gerrit_core::domain::GerritRepository;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use subtle::ConstantTimeEq;

use crate::config::Config;
use crate::health::{health_handler, metrics_handler, ready_handler};
use crate::mcp::GerritServer;

/// Runs the MCP server over Streamable HTTP with health/metrics endpoints.
pub async fn run_http<R: GerritRepository + Send + Sync + 'static>(
    config: &Config,
    server: GerritServer<R>,
) -> anyhow::Result<()> {
    let service_factory = {
        let svr = server.clone();
        move || Ok(svr.clone())
    };

    let mut server_config = StreamableHttpServerConfig::default().with_legacy_session_mode(false);
    if !config.transport.allowed_hosts.is_empty() {
        server_config = server_config.with_allowed_hosts(&config.transport.allowed_hosts);
    }

    let mcp_service = StreamableHttpService::new(
        service_factory,
        Arc::new(NeverSessionManager::default()),
        server_config,
    );

    let health_path = config.transport.health_path.clone();
    let ready_path = config.transport.ready_path.clone();
    let metrics_path = config.transport.metrics_path.clone();
    let http_path = config.transport.http_path.clone();
    let bind_addr = config.transport.bind_addr.clone();
    let mcp_auth_token = config.transport.mcp_auth_token.clone();

    let app = build_app(
        Router::new().nest_service(&http_path, mcp_service),
        &health_path,
        &ready_path,
        &metrics_path,
        &mcp_auth_token,
    );

    tracing::info!(%bind_addr, %http_path, "starting Streamable HTTP transport");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}

/// Builds the axum app: MCP routes (optionally token-protected) plus public
/// health/ready/metrics endpoints.
fn build_app(
    mcp_routes: Router,
    health_path: &str,
    ready_path: &str,
    metrics_path: &str,
    mcp_auth_token: &str,
) -> Router {
    let mut mcp_routes = mcp_routes;
    if !mcp_auth_token.is_empty() {
        let token: Arc<str> = mcp_auth_token.into();
        let middleware_fn = move |req: Request, next: Next| {
            let token = Arc::clone(&token);
            mcp_token_auth(req, next, token)
        };
        mcp_routes = mcp_routes.layer(middleware::from_fn(middleware_fn));
        tracing::info!("MCP token auth enabled");
    }

    Router::new()
        .merge(mcp_routes)
        .route(health_path, get(health_handler))
        .route(ready_path, get(ready_handler))
        .route(metrics_path, get(metrics_handler))
}

async fn mcp_token_auth(
    req: Request,
    next: Next,
    expected_token: Arc<str>,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(provided) = auth_header else {
        tracing::warn!("MCP token auth: missing Authorization header");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if provided.as_bytes().ct_ne(expected_token.as_bytes()).into() {
        tracing::warn!("MCP token auth: invalid token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::header::AUTHORIZATION;
    use axum::http::{Request, StatusCode};
    use axum::middleware;
    use axum::routing::get;
    use tower::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn make_app(token: &str) -> Router {
        let token: Arc<str> = token.into();
        let middleware_fn = move |req: Request<Body>, next: Next| {
            let token = Arc::clone(&token);
            mcp_token_auth(req, next, token)
        };
        Router::new()
            .route("/", get(ok_handler))
            .layer(middleware::from_fn(middleware_fn))
    }

    #[tokio::test]
    async fn mcp_token_auth_valid_token_passes() {
        let app = make_app("secret-token");
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(AUTHORIZATION, "Bearer secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn mcp_token_auth_wrong_token_returns_401() {
        let app = make_app("secret-token");
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(AUTHORIZATION, "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mcp_token_auth_missing_header_returns_401() {
        let app = make_app("secret-token");
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn token_auth_protects_mcp_but_keeps_health_public() {
        let app = build_app(
            Router::new().route("/mcp", get(ok_handler)),
            "/healthz",
            "/readyz",
            "/metrics",
            "secret-token",
        );

        let health_no_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_no_token.status(), StatusCode::OK);

        let mcp_no_token = app
            .clone()
            .oneshot(Request::builder().uri("/mcp").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(mcp_no_token.status(), StatusCode::UNAUTHORIZED);

        let mcp_with_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/mcp")
                    .header(AUTHORIZATION, "Bearer secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(mcp_with_token.status(), StatusCode::OK);
    }
}
