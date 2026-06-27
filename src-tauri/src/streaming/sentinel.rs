pub const TOOL_PREFIX: &str = "\u{FFFE}tool:";
pub const ARGS_PREFIX: &str = "\u{FFFE}args:";
pub const REASONING_PREFIX: &str = "\u{FFFE}reasoning:";
pub const DONE_PREFIX: &str = "\u{FFFE}done:";
pub const STATS_PREFIX: &str = "\u{FFFE}stats:";

pub fn strip_prefix<'a>(chunk: &'a str, prefix: &str) -> Option<&'a str> {
    chunk.strip_prefix(prefix)
}
