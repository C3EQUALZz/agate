//! The proxy data-plane metrics port: counters hidden behind an interface so
//! the presentation layer records through application logic, not the `metrics`
//! macros directly. The composition root supplies a real adapter; tests a fake.

/// The outcome of inspecting one event — the `outcome` label on the
/// `agate_events_inspected_total` counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionOutcome {
    /// Forwarded unchanged.
    Forward,
    /// Held back until the buffered logical unit completes.
    Buffer,
    /// Forwarded after a policy transform (e.g. redaction).
    Transform,
    /// Denied and dropped.
    Deny,
    /// Terminated the run.
    Terminate,
}

impl InspectionOutcome {
    /// The stable metric-label value for this outcome.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Buffer => "buffer",
            Self::Transform => "transform",
            Self::Deny => "deny",
            Self::Terminate => "terminate",
        }
    }
}

/// Records proxy data-plane metrics.
pub trait ProxyMetrics: Send + Sync {
    /// A run was received and forwarded to the upstream agent.
    fn record_run(&self);

    /// An upstream request or response stream failed.
    fn record_upstream_error(&self);

    /// An inspected event resolved to `outcome`.
    fn record_inspected(&self, outcome: InspectionOutcome);
}
