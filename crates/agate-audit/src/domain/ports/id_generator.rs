/// Domain port: generates fresh identities of type `Id` (e.g. `LogId`,
/// `EventId`). Generation is I/O (randomness), implemented in an adapter.
pub trait IdGenerator<Id>: Send + Sync {
    fn generate(&self) -> Id;
}
