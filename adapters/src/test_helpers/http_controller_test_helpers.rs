use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tokio::{sync::mpsc, task::JoinHandle};
use tower::ServiceExt;

use super::*;
use crate::{
    http::{dto::ReplyDto, routes::router},
    response_body::{body_json, body_text},
    state::{DaemonCommand, DaemonOutcome},
};

const CHANNEL_DEPTH: usize = 1;

pub fn reply_of(body: &str) -> ReplyDto {
    serde_json::from_str(body).expect("an accepted request answers with a reply envelope")
}

pub struct Exchange {
    pub status: StatusCode,
    pub body: String,
    pub request: Option<DaemonRequest>,
}

pub fn get_from(path: &str) -> Request<Body> {
    built(path, "GET", Body::empty())
}

pub fn post_to(path: &str, body: &str) -> Request<Body> {
    built(path, "POST", Body::from(body.to_string()))
}

pub fn delete_at(path: &str) -> Request<Body> {
    built(path, "DELETE", Body::empty())
}

pub async fn exchange(outcome: DaemonOutcome, request: Request<Body>) -> Exchange {
    let (sender, mut receiver) = mpsc::channel(CHANNEL_DEPTH);
    let actor: JoinHandle<DaemonRequest> = tokio::spawn(async move {
        let DaemonCommand {
            request: asked,
            reply,
        } = receiver.recv().await.expect("should receive one command");
        reply.send(outcome).expect("should answer");
        asked
    });
    let (status, body) = drive(sender, request).await;
    Exchange {
        status,
        body,
        request: Some(actor.await.expect("actor")),
    }
}

pub async fn exchange_without_a_daemon(request: Request<Body>) -> Exchange {
    let (sender, receiver) = mpsc::channel(CHANNEL_DEPTH);
    drop(receiver);
    let (status, body) = drive(sender, request).await;
    Exchange {
        status,
        body,
        request: None,
    }
}

pub async fn exchange_with_an_abandoned_request(request: Request<Body>) -> Exchange {
    let (sender, mut receiver) = mpsc::channel(CHANNEL_DEPTH);
    let actor = tokio::spawn(async move {
        let DaemonCommand {
            request: _,
            reply: _,
        } = receiver.recv().await.expect("should receive one command");
    });
    let (status, body) = drive(sender, request).await;
    actor.await.expect("actor");
    Exchange {
        status,
        body,
        request: None,
    }
}

pub async fn exchange_json(request: Request<Body>) -> (StatusCode, Value) {
    let (sender, _receiver) = mpsc::channel(CHANNEL_DEPTH);
    let response = answered(sender, request).await;
    let status = response.status();
    (status, body_json(response.into_body()).await)
}

async fn drive(
    sender: mpsc::Sender<DaemonCommand>,
    request: Request<Body>,
) -> (StatusCode, String) {
    let response = answered(sender, request).await;
    let status = response.status();
    (status, body_text(response.into_body()).await)
}

async fn answered(
    sender: mpsc::Sender<DaemonCommand>,
    request: Request<Body>,
) -> axum::response::Response {
    router(DaemonHandle::new(sender))
        .oneshot(request)
        .await
        .expect("router should answer")
}

fn built(path: &str, method: &str, body: Body) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json")
        .body(body)
        .expect("should build a request")
}
