/// One decoded Server-Sent Event.
///
/// `data` is the concatenated payload (for AG-UI, the event JSON). `raw` keeps
/// the exact bytes of the event block as received, so an *allowed* event is
/// forwarded byte-for-byte; a *transformed* one is re-encoded instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    pub data: String,
    pub event: Option<String>,
    pub id: Option<String>,
    pub raw: String,
}
