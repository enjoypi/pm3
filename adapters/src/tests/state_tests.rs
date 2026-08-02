use tokio::task::JoinHandle;
use usecases::{AppSelector, SupervisionReply, SupervisionRequest, UsecaseError};

use super::*;
use crate::process_views::running_view;

const CHANNEL_DEPTH: usize = 1;

fn actor_replying(outcome: SupervisionOutcome) -> (DaemonHandle, JoinHandle<SupervisionRequest>) {
    let (sender, mut receiver) = mpsc::channel(CHANNEL_DEPTH);
    let actor = tokio::spawn(async move {
        let command = receiver.recv().await.expect("should receive one command");
        let DaemonCommand { request, reply } = command;
        reply.send(outcome).expect("should answer");
        request
    });
    (DaemonHandle::new(sender), actor)
}

fn actor_dropping_the_reply() -> (DaemonHandle, JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::channel(CHANNEL_DEPTH);
    let actor = tokio::spawn(async move {
        let command = receiver.recv().await.expect("should receive one command");
        let DaemonCommand {
            request: _,
            reply: _,
        } = command;
    });
    (DaemonHandle::new(sender), actor)
}

#[tokio::test]
async fn send_hands_the_request_to_the_daemon() {
    let (handle, actor) = actor_replying(Ok(SupervisionReply::Listed(Vec::new())));
    handle
        .send(SupervisionRequest::List)
        .await
        .expect("should send");
    assert_eq!(actor.await.expect("actor"), SupervisionRequest::List);
}

#[tokio::test]
async fn send_hands_over_a_selector_untouched() {
    let selector = AppSelector::Name("web".to_string());
    let (handle, actor) = actor_replying(Ok(SupervisionReply::Stopped {
        name: "web".to_string(),
    }));
    handle
        .send(SupervisionRequest::Stop(selector.clone()))
        .await
        .expect("should send");
    assert_eq!(
        actor.await.expect("actor"),
        SupervisionRequest::Stop(selector)
    );
}

#[tokio::test]
async fn send_returns_the_reply_from_the_daemon() {
    let views = vec![running_view(0, "web")];
    let (handle, _actor) = actor_replying(Ok(SupervisionReply::Listed(views.clone())));
    let reply = handle
        .send(SupervisionRequest::List)
        .await
        .expect("should send");
    assert_eq!(reply, SupervisionReply::Listed(views));
}

#[tokio::test]
async fn send_propagates_a_usecase_failure() {
    let (handle, _actor) = actor_replying(Err(UsecaseError::NotFound("web".to_string()).into()));
    let err = handle
        .send(SupervisionRequest::List)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot find app 'web'"), "got: {err}");
}

#[tokio::test]
async fn send_reports_a_daemon_that_stopped_accepting_commands() {
    let (sender, receiver) = mpsc::channel(CHANNEL_DEPTH);
    drop(receiver);
    let err = DaemonHandle::new(sender)
        .send(SupervisionRequest::List)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no longer accepting commands"), "got: {err}");
}

#[tokio::test]
async fn send_reports_a_request_the_daemon_abandoned() {
    let (handle, actor) = actor_dropping_the_reply();
    let err = handle
        .send(SupervisionRequest::List)
        .await
        .unwrap_err()
        .to_string();
    actor.await.expect("actor");
    assert!(err.contains("dropped the request"), "got: {err}");
}
