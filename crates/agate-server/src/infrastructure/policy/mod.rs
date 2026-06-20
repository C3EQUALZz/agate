//! The policy bridge: adapts the `agate-policy` context to the proxy's
//! `PolicyPort`, translating between the two contexts' vocabularies. The static
//! ruleset and the optional CEL engine are both `PolicyPort` backends sharing
//! the [`projection`] lift.

pub mod adapter;
#[cfg(feature = "policy-cel")]
pub mod cel_adapter;
mod projection;

pub use adapter::PolicyAdapter;
#[cfg(feature = "policy-cel")]
pub use cel_adapter::CelPolicyAdapter;
