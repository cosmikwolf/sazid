use async_openai::types::Role;
use crate::app::format_chat_message;

pub struct Message {
    pub role: Role,
    pub content: String,
}

// a display function for Message
impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_chat_message(f, self.role.clone(), self.content.clone())
    }
}

