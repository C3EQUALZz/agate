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
    /// it reached the handler (e.g. the outbox shed it under backpressure).
    fn record_dropped(&self);

    /// Report how full the composition root's audit outbox is (`used` of
    /// `capacity` slots), so operators can see backpressure building before the
    /// tamper-evident log starts to stall or shed.
    fn observe_outbox_depth(&self, used: usize, capacity: usize);
}
