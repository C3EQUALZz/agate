use crate::domain::common::values::ValueObject;

/// A run/step lifecycle transition, abstracted away from any one protocol's
/// event names (AG-UI `RUN_STARTED`/`STEP_STARTED`/… map onto these).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecyclePhase {
    RunStarted,
    RunFinished,
    RunError,
    StepStarted(String),
    StepFinished(String),
}

impl ValueObject for LifecyclePhase {}
