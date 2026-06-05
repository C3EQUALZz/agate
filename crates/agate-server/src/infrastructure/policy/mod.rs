//! The policy bridge: adapts the `agate-policy` context to the proxy's
//! `PolicyPort`, translating between the two contexts' vocabularies.

pub mod adapter;

pub use adapter::PolicyAdapter;
