pub fn compose_system_prompt(
    persona: &str,
    memory_block: Option<&str>,
    tools_block: &str,
) -> String {
    let mut prompt = String::from(persona);
    if let Some(memory) = memory_block.filter(|memory| !memory.trim().is_empty()) {
        prompt.push_str("\n\nMemory:\n");
        prompt.push_str(memory);
    }
    prompt.push_str("\n\nTools:\n");
    prompt.push_str(tools_block);
    prompt
}
