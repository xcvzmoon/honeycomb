use std::collections::HashMap;

#[derive(Default)]
pub struct AgentTaskState {
    pub read_cache: HashMap<String, serde_json::Value>,
    pub consecutive_directory_listings: u32,
}

impl AgentTaskState {
    pub fn reset_wandering_after_file_read(&mut self) {
        self.consecutive_directory_listings = 0;
    }
}
