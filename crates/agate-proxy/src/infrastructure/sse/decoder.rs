use super::event::SseEvent;

/// Incremental, order-preserving Server-Sent Events decoder.
///
/// Fed arbitrary byte chunks from the streaming HTTP body via [`push`], it emits
/// each event once its terminating blank line arrives, buffering partial events
/// across chunks. Follows the WHATWG SSE parsing rules for `data`/`event`/`id`
/// fields, comments (`:` lines), and `\n`/`\r\n` terminators; each event keeps
/// its exact received bytes in [`SseEvent::raw`].
///
/// [`push`]: SseDecoder::push
#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    raw: Vec<u8>,
    data: String,
    event: Option<String>,
    id: Option<String>,
    has_data: bool,
}

impl SseDecoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk; return the events that became complete.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        while let Some(newline) = self.buffer.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=newline).collect();
            self.raw.extend_from_slice(&line);

            let mut content = &line[..line.len() - 1]; // drop the '\n'
            if content.last() == Some(&b'\r') {
                content = &content[..content.len() - 1];
            }

            if content.is_empty() {
                if let Some(event) = self.dispatch() {
                    events.push(event);
                }
            } else if content.first() != Some(&b':') {
                self.parse_field(content);
            }
            // a comment line (leading ':') stays in `raw` but carries no field
        }

        events
    }

    fn parse_field(&mut self, content: &[u8]) {
        let line = String::from_utf8_lossy(content);
        let (field, value) = match line.find(':') {
            Some(colon) => {
                let value = &line[colon + 1..];
                (&line[..colon], value.strip_prefix(' ').unwrap_or(value))
            }
            None => (line.as_ref(), ""),
        };

        match field {
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
                self.has_data = true;
            }
            "event" => self.event = Some(value.to_owned()),
            "id" => self.id = Some(value.to_owned()),
            _ => {}
        }
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        let raw = String::from_utf8_lossy(&std::mem::take(&mut self.raw)).into_owned();
        let data = std::mem::take(&mut self.data);
        let event = self.event.take();
        let id = self.id.take();
        let had_data = std::mem::replace(&mut self.has_data, false);

        if !had_data {
            return None;
        }

        let mut data = data;
        if data.ends_with('\n') {
            data.pop(); // strip the single trailing newline per the SSE rules
        }
        Some(SseEvent {
            data,
            event,
            id,
            raw,
        })
    }
}
