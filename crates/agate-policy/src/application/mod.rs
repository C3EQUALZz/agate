//! Application layer of the policy context: the service that holds the active
//! ruleset and decides each action. Pure and synchronous — there are no
//! outbound ports (the ruleset is supplied at construction); the proxy reaches
//! it through its own async `PolicyPort`, adapted at the composition root.

pub mod decide;

pub use decide::PolicyService;
