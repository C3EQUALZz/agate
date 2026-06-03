//! Read models returned by query handlers (decoupled from domain entities).

pub mod consistency_proof_view;
pub mod inclusion_proof_view;

pub use consistency_proof_view::ConsistencyProofView;
pub use inclusion_proof_view::InclusionProofView;
