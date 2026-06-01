/// Marker trait for immutable, value-compared domain types.
pub trait ValueObject: Clone + PartialEq + Eq {}
