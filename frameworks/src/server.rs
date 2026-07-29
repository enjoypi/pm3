use std::{
    future::{Future, IntoFuture},
    pin::Pin,
    time::Duration,
};

use adapters::{AppState, ExampleStore, ServerConfig, examples, handlers, middleware};
use axum::{
    Router,
    routing::{get, post},
};
use thiserror::Error;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

type ServeFuture = Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>>;
type ShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("cannot bind to {addr}: {source}")]
    Bind {
        addr: String,
        source: std::io::Error,
    },
    #[error("cannot serve requests: {0}")]
    Serve(#[source] std::io::Error),
}

pub fn build_router(state: AppState) -> Router {
    apply_layers(probe_router(state))
}

pub fn build_router_with_examples<S>(state: AppState, store: S) -> Router
where
    S: ExampleStore + Clone + 'static,
{
    apply_layers(probe_router(state).merge(example_router(store)))
}

fn probe_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/readiness", get(handlers::readiness))
        .with_state(state)
}

fn example_router<S>(store: S) -> Router
where
    S: ExampleStore + Clone + 'static,
{
    Router::new()
        .route("/examples", post(examples::create::<S>))
        .route("/examples/{id}", get(examples::find::<S>))
        .with_state(store)
}

fn apply_layers(router: Router) -> Router {
    router
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(TraceLayer::new_for_http())
}

pub async fn start_server(
    cfg: &ServerConfig,
    router: Router,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    start_server_inner(cfg, router, Box::pin(shutdown_signal), drain_timeout).await
}

async fn start_server_inner(
    cfg: &ServerConfig,
    router: Router,
    shutdown_signal: ShutdownFuture,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = TcpListener::bind(&addr)
        .await
        .inspect_err(|e| log_bind_failure(&addr, e))
        .map_err(|e| ServerError::Bind {
            addr: addr.clone(),
            source: e,
        })?;
    serve_listener_inner(listener, router, shutdown_signal, drain_timeout).await
}

fn log_bind_failure(addr: &str, error: &std::io::Error) {
    tracing::error!(
        feature = "server",
        operation = "bind",
        result = "error",
        addr = %addr,
        error = %error,
        "cannot bind listener address",
    );
}

pub async fn serve_listener(
    listener: TcpListener,
    router: Router,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    serve_listener_inner(listener, router, Box::pin(shutdown_signal), drain_timeout).await
}

async fn serve_listener_inner(
    listener: TcpListener,
    router: Router,
    shutdown_signal: ShutdownFuture,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    let local_addr = listener
        .local_addr()
        .expect("internal error: local_addr is infallible on a bound tokio TcpListener");

    tracing::info!(
        feature = "server",
        operation = "listen",
        result = "ok",
        addr = %local_addr,
        "server listening",
    );

    let (drain_started_tx, drain_started_rx) = tokio::sync::oneshot::channel::<()>();
    let signal_with_notify = async move {
        shutdown_signal.await;
        let _ = drain_started_tx.send(());
    };

    let serve: ServeFuture = Box::pin(
        axum::serve(listener, router)
            .with_graceful_shutdown(signal_with_notify)
            .into_future(),
    );

    serve_with_drain_timeout(serve, drain_started_rx, drain_timeout).await
}

async fn serve_with_drain_timeout(
    mut serve: ServeFuture,
    drain_started_rx: tokio::sync::oneshot::Receiver<()>,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    tokio::select! {
        result = &mut serve => return result.map_err(ServerError::Serve),
        _ = drain_started_rx => {}
    }

    let drain_secs = drain_timeout.as_secs();
    tracing::info!(
        feature = "server",
        operation = "drain.start",
        result = "draining",
        drain_timeout_secs = drain_secs,
        "draining connections",
    );

    #[expect(
        clippy::single_match_else,
        clippy::option_if_let_else,
        reason = "closure 形态会被 llvm-cov 当作独立未覆盖 fn"
    )]
    match tokio::time::timeout(drain_timeout, &mut serve).await {
        Ok(result) => result.map_err(ServerError::Serve),
        Err(_) => {
            tracing::warn!(
                feature = "server",
                operation = "drain.timeout",
                result = "timeout",
                drain_timeout_secs = drain_secs,
                "drain timeout exceeded; forcing shutdown",
            );
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "tests/example_route_tests.rs"]
mod example_route_tests;
#[cfg(test)]
#[path = "tests/server_tests.rs"]
mod tests;
