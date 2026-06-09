use super::postgres_config::PostgresConfig;

/// Backend-neutral descriptor of which persistence backend to use and how to
/// connect to it. The composition root maps its on-disk config onto this; the
/// audit context stays free of any file format.
///
/// Adding a backend = a new variant here, a [`Storage`](crate::setup::storage::Storage)
/// arm that connects it, and a provider module that registers its adapters.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum StorageConfig {
    /// PostgreSQL (the only implemented backend).
    Postgres(PostgresConfig),
}
