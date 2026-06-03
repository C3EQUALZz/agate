use std::sync::Arc;

use async_trait::async_trait;

use crate::application::common::messaging::{Behavior, Command, Next, Request};
use crate::application::common::ports::TransactionManager;
use crate::application::errors::AuditError;

/// Pipeline behavior that runs a **command** inside a transaction: commit on
/// success, rollback on failure. Registered conditionally at the composition
/// root, so commands are transactional only when this behavior is in the pipeline.
///
/// Applies to any `Command` whose response is `Result<_, AuditError>`.
pub struct TransactionBehavior {
    transaction: Arc<dyn TransactionManager>,
}

impl TransactionBehavior {
    pub fn new(transaction: Arc<dyn TransactionManager>) -> Self {
        Self { transaction }
    }
}

#[async_trait]
impl<R, T> Behavior<R> for TransactionBehavior
where
    R: Command + Request<Response = Result<T, AuditError>>,
    T: Send + 'static,
{
    async fn handle(&self, request: R, next: Next<R>) -> R::Response {
        match next.call(request).await {
            Ok(value) => match self.transaction.commit().await {
                Ok(()) => Ok(value),
                Err(commit_error) => Err(commit_error),
            },
            Err(error) => {
                // Best-effort rollback; preserve the handler's original error.
                let _ = self.transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::common::messaging::{Mediator, RequestHandler};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingTransaction {
        commits: AtomicUsize,
        rollbacks: AtomicUsize,
    }

    #[async_trait]
    impl TransactionManager for CountingTransaction {
        async fn commit(&self) -> Result<(), AuditError> {
            self.commits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn rollback(&self) -> Result<(), AuditError> {
            self.rollbacks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct Succeed;
    impl Request for Succeed {
        type Response = Result<u32, AuditError>;
    }
    impl Command for Succeed {}

    struct SucceedHandler;
    #[async_trait]
    impl RequestHandler<Succeed> for SucceedHandler {
        async fn handle(&self, _request: Succeed) -> Result<u32, AuditError> {
            Ok(42)
        }
    }

    struct Fail;
    impl Request for Fail {
        type Response = Result<u32, AuditError>;
    }
    impl Command for Fail {}

    struct FailHandler;
    #[async_trait]
    impl RequestHandler<Fail> for FailHandler {
        async fn handle(&self, _request: Fail) -> Result<u32, AuditError> {
            Err(AuditError::Storage("boom".to_string()))
        }
    }

    #[tokio::test]
    async fn commits_on_success() {
        let tx = Arc::new(CountingTransaction::default());
        let behaviors: Vec<Arc<dyn Behavior<Succeed>>> =
            vec![Arc::new(TransactionBehavior::new(tx.clone()))];
        let mediator = Mediator::new(Arc::new(SucceedHandler), behaviors);

        assert_eq!(mediator.send(Succeed).await.unwrap(), 42);
        assert_eq!(tx.commits.load(Ordering::SeqCst), 1);
        assert_eq!(tx.rollbacks.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rolls_back_on_error() {
        let tx = Arc::new(CountingTransaction::default());
        let behaviors: Vec<Arc<dyn Behavior<Fail>>> =
            vec![Arc::new(TransactionBehavior::new(tx.clone()))];
        let mediator = Mediator::new(Arc::new(FailHandler), behaviors);

        assert!(mediator.send(Fail).await.is_err());
        assert_eq!(tx.commits.load(Ordering::SeqCst), 0);
        assert_eq!(tx.rollbacks.load(Ordering::SeqCst), 1);
    }
}
