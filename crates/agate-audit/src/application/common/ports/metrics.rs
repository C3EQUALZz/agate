/// Records transparency-log append outcomes.
///
/// An application port so metric emission is hidden behind an interface: the
/// [`MetricsBehavior`](crate::application::common::behaviors::MetricsBehavior)
/// records the outcome of every append through it, the composition root supplies
/// a real adapter over the `metrics` facade, and tests substitute a fake.
pub trait AuditMetrics: Send + Sync {
    /// A record was durably appended to the transparency log.
    fn record_appended(&self);

    /// A record was not appended — a handler error, or a record dropped before
    /// it reached the handler (e.g. outbox backpressure at the composition root).
    fn record_dropped(&self);
}
