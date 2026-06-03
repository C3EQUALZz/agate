use agate_audit::domain::merkle::TransparencyLogFactory;

use super::crypto::sha256;

pub fn log_factory() -> TransparencyLogFactory {
    TransparencyLogFactory::new(sha256())
}
