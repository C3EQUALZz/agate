use crate::domain::common::values::ValueObject;

/// Identifier of the key that produced a [`Signature`](super::Signature).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyId(pub String);

impl ValueObject for KeyId {}
