use crate::streaming::ChatEvent;
use crate::streaming::sentinel;

pub fn map_chunk(chunk: &str) -> Option<ChatEvent> {
    if let Some(content) = sentinel::strip_prefix(chunk, sentinel::REASONING_PREFIX) {
        return Some(ChatEvent::Reasoning {
            content: content.to_string(),
        });
    }

    if chunk.starts_with('\u{FFFE}') {
        return None;
    }

    Some(ChatEvent::Text {
        content: chunk.to_string(),
    })
}
