use std::hash::Hash;

/// Contract for domain entities: a stable identity.
pub trait Entity {
    type Id: Clone + Eq + Hash;

    fn id(&self) -> &Self::Id;
}
