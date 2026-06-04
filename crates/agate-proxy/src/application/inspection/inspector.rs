use std::sync::Arc;

use super::action::InspectionAction;
use super::context::InspectionContext;
use crate::application::common::ports::{AuditSink, PolicyPort};
use crate::domain::inspection::{Fragment, Run, StructuralOutcome, Verdict};

/// Drives one fragment through the inspection seam: the pure-domain [`Run`]
/// state machine decides the structural outcome, then a complete semantic event
/// is judged by the (async) [`PolicyPort`] and recorded to the [`AuditSink`].
/// Transport concerns (the raw-frame buffer) stay in the presentation layer —
/// the [`InspectionAction`] only says *what* to do.
///
/// A structural reject is a protocol-integrity violation, so it terminates the
/// run (fail-closed); content denials from the policy only drop the one event.
pub struct Inspector {
    policy: Arc<dyn PolicyPort>,
    audit: Arc<dyn AuditSink>,
}

impl Inspector {
    pub fn new(policy: Arc<dyn PolicyPort>, audit: Arc<dyn AuditSink>) -> Self {
        Self { policy, audit }
    }

    pub async fn inspect(
        &self,
        run: &mut Run,
        context: &InspectionContext,
        fragment: Fragment,
    ) -> InspectionAction {
        match run.inspect(fragment) {
            StructuralOutcome::Buffering => InspectionAction::Hold,
            StructuralOutcome::Reject(reason) => InspectionAction::Terminate(reason),
            StructuralOutcome::Ready(event) => {
                let verdict = self.policy.decide(context, &event).await;
                self.audit.record(context, &event, &verdict).await;
                match verdict {
                    Verdict::Allow => InspectionAction::Forward,
                    Verdict::Transform(replacement) => {
                        InspectionAction::ForwardTransformed(replacement)
                    }
                    Verdict::Deny(reason) => InspectionAction::Drop(reason),
                    Verdict::Terminate(reason) => InspectionAction::Terminate(reason),
                    Verdict::Buffer => InspectionAction::Hold,
                }
            }
        }
    }
}
