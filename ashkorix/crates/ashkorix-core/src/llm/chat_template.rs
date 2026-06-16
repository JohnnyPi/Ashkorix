use crate::error::{AshkorixError, Result};
use crate::traits::model::ChatMessage;
use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate, LlamaModel};

pub fn format_messages_with_model(
    model: &LlamaModel,
    template: &LlamaChatTemplate,
    messages: &[ChatMessage],
) -> Result<String> {
    let llama_messages: Vec<LlamaChatMessage> = messages
        .iter()
        .map(|m| {
            LlamaChatMessage::new(m.role.clone(), m.content.clone())
                .map_err(|e| AshkorixError::Model(e.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;

    model
        .apply_chat_template(template, &llama_messages, true)
        .map_err(|e| AshkorixError::Model(e.to_string()))
}
