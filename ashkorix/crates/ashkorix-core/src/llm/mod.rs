pub mod backend;
pub mod chat_template;
pub mod gpu;
pub mod service;

pub use chat_template::format_messages_with_model;
pub use gpu::{cuda_status, resolve_gpu_layers, CudaStatus};
pub use service::LlamaModelService;
