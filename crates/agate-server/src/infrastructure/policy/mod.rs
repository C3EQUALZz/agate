//! The policy bridge: adapts the `agate-policy` context to the proxy's
//! `PolicyPort`, translating between the two contexts' vocabularies. The static
//! ruleset and the optional CEL engine are both `PolicyPort` backends sharing
//! the `projection` lift.

pub mod adapter;
#[cfg(feature = "policy-cel")]
pub mod cel_adapter;
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
mod event_view;
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
pub mod policy_watch;
mod projection;
#[cfg(feature = "policy-rego")]
pub mod rego_adapter;

pub use adapter::PolicyAdapter;
#[cfg(feature = "policy-cel")]
pub use cel_adapter::CelPolicyAdapter;
#[cfg(feature = "policy-rego")]
pub use rego_adapter::RegoPolicyAdapter;

/// A policy engine whose rules live in a file and can be recompiled in place at
/// runtime (the `SIGHUP` reload and the file-watch share this). Implemented by
/// the CEL and Rego adapters; the composition root drives reloads through it
/// without knowing which engine is behind it.
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
pub trait ReloadablePolicy: Send + Sync + 'static {
    /// Re-read and recompile the policy file, swapping it in on success. Fail-safe
    /// by contract: on any error the running policy is kept, and the error is
    /// returned for the caller to log.
    fn reload_policy(&self) -> Result<(), String>;
}
