use crate::domain::common::values::ValueObject;

/// Kind of opaque event the proxy cannot inspect (AG-UI `RAW`, `CUSTOM`, and
/// the `encryptedValue` / reasoning-encrypted payloads). The decision is
/// pass-through-or-drop; the content is never trusted or parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpaqueKind {
    /// A foreign/underlying event wrapped verbatim (AG-UI `RAW`).
    Raw,
    /// An arbitrary application-defined event (AG-UI `CUSTOM`).
    Custom,
    /// A provider-issued encrypted/opaque blob (reasoning or message).
    Encrypted,
}

impl ValueObject for OpaqueKind {}
