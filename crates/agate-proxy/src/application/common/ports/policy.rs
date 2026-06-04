use async_trait::async_trait;

use crate::application::inspection::InspectionContext;
use crate::domain::inspection::{AgentEvent, Verdict};

/// Decides the content/authorization verdict for a complete semantic event —
/// the async half of the seam (a policy may consult external services). Returns
/// the final [`Verdict`], including any `Transform` replacement.
///
/// The proxy ships a trivial allow-all implementation until `agate-policy`
/// provides a real one.
#[async_trait]
pub trait PolicyPort: Send + Sync {
    async fn decide(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent>;
}
