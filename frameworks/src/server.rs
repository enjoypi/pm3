use std::{
    fmt::Debug,
    future::{Future, IntoFuture},
    pin::Pin,
    time::Duration,
};

use axum::{Router, serve::Listener};
use thiserror::Error;

type ServeFuture = Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>>;
type ShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("cannot serve requests: {0}")]
    Serve(#[source] std::io::Error),
}

pub async fn serve_listener<L>(
    listener: L,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
    drain_timeout: Duration,
) -> Result<(), ServerError>
where
    L: Listener + Send + 'static,
    L::Addr: Debug,
{
    let addr = listener.local_addr().ok();
    tracing::info!(
        feature = "server",
        operation = "listen",
        result = "ok",
        ?addr,
        "pm3 daemon listening",
    );
    serve_until_drained(listener, router, Box::pin(shutdown), drain_timeout).await
}

async fn serve_until_drained<L>(
    listener: L,
    router: Router,
    shutdown: ShutdownFuture,
    drain_timeout: Duration,
) -> Result<(), ServerError>
where
    L: Listener + Send + 'static,
    L::Addr: Debug,
{
    let (drain_started, drain_watch) = tokio::sync::oneshot::channel::<()>();
    let signal_with_notify = async move {
        shutdown.await;
        drain_started.send(()).ok();
    };
    let serve: ServeFuture = Box::pin(
        axum::serve(listener, router)
            .with_graceful_shutdown(signal_with_notify)
            .into_future(),
    );
    drain(serve, drain_watch, drain_timeout).await
}

async fn drain(
    mut serve: ServeFuture,
    drain_watch: tokio::sync::oneshot::Receiver<()>,
    drain_timeout: Duration,
) -> Result<(), ServerError> {
    tokio::select! {
        result = &mut serve => return result.map_err(ServerError::Serve),
        _ = drain_watch => {}
    }

    let drain_timeout_secs = drain_timeout.as_secs();
    tracing::info!(
        feature = "server",
        operation = "drain.start",
        result = "draining",
        drain_timeout_secs,
        "draining connections",
    );

    match tokio::time::timeout(drain_timeout, &mut serve).await {
        Ok(result) => result.map_err(ServerError::Serve),
        Err(_elapsed) => {
            tracing::warn!(
                feature = "server",
                operation = "drain.timeout",
                result = "timeout",
                drain_timeout_secs,
                "drain timeout exceeded; forcing shutdown",
            );
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "tests/server_tests.rs"]
mod tests;
