use std::sync::Arc;

use super::behavior::{Behavior, Next};
use super::handler::RequestHandler;
use super::request::Request;

/// Sends a request through its behavior pipeline to the handler.
///
/// `behaviors` run in order — the first is outermost (runs first, returns last).
/// Built per request type, typically by the DI container at the composition
/// root: behaviors are registered conditionally (by TOML/feature), so a
/// disabled concern simply isn't in the list.
pub struct Mediator<R: Request> {
    handler: Arc<dyn RequestHandler<R>>,
    behaviors: Vec<Arc<dyn Behavior<R>>>,
}

impl<R: Request> Mediator<R> {
    pub fn new(handler: Arc<dyn RequestHandler<R>>, behaviors: Vec<Arc<dyn Behavior<R>>>) -> Self {
        Self { handler, behaviors }
    }

    pub fn without_behaviors(handler: Arc<dyn RequestHandler<R>>) -> Self {
        Self {
            handler,
            behaviors: Vec::new(),
        }
    }

    pub async fn send(&self, request: R) -> R::Response {
        // Innermost link: the handler itself.
        let handler = self.handler.clone();
        let mut next = Next::new(Box::new(move |req| {
            Box::pin(async move { handler.handle(req).await })
        }));

        // Wrap from the last behavior to the first, so the first is outermost.
        for behavior in self.behaviors.iter().rev() {
            let behavior = behavior.clone();
            let inner = next;
            next = Next::new(Box::new(move |req| {
                Box::pin(async move { behavior.handle(req, inner).await })
            }));
        }

        next.call(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::Mediator;
    use crate::application::common::messaging::{Behavior, Command, Next, Request, RequestHandler};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct Ping(String);
    impl Request for Ping {
        type Response = String;
    }
    impl Command for Ping {}

    struct PingHandler;
    #[async_trait]
    impl RequestHandler<Ping> for PingHandler {
        async fn handle(&self, request: Ping) -> String {
            format!("pong:{}", request.0)
        }
    }

    struct Tag {
        tag: &'static str,
        log: Arc<Mutex<Vec<String>>>,
    }
    #[async_trait]
    impl Behavior<Ping> for Tag {
        async fn handle(&self, request: Ping, next: Next<Ping>) -> String {
            self.log.lock().unwrap().push(format!("enter:{}", self.tag));
            let response = next.call(request).await;
            self.log.lock().unwrap().push(format!("exit:{}", self.tag));
            format!("[{}]{response}", self.tag)
        }
    }

    #[tokio::test]
    async fn handler_runs_with_no_behaviors() {
        let mediator = Mediator::without_behaviors(Arc::new(PingHandler));
        assert_eq!(mediator.send(Ping("x".into())).await, "pong:x");
    }

    #[tokio::test]
    async fn behaviors_wrap_handler_in_declared_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mediator = Mediator::new(
            Arc::new(PingHandler),
            vec![
                Arc::new(Tag {
                    tag: "a",
                    log: log.clone(),
                }),
                Arc::new(Tag {
                    tag: "b",
                    log: log.clone(),
                }),
            ],
        );

        let response = mediator.send(Ping("x".into())).await;

        assert_eq!(response, "[a][b]pong:x");
        assert_eq!(
            *log.lock().unwrap(),
            vec!["enter:a", "enter:b", "exit:b", "exit:a"],
        );
    }
}
