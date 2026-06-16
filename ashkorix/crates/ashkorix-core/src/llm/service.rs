use crate::error::{AshkorixError, Result};
use crate::llm::backend::shared_llama_backend;
use crate::llm::chat_template::format_messages_with_model;
use crate::traits::model::{
    ChatMessage, GenerateParams, LoadOptions, ModelInfo, TokenEvent,
};
use crate::traits::ModelService;
use async_trait::async_trait;
use futures::Stream;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::ggml_time_us;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::TokenToStringError;
use llama_cpp_2::sampling::LlamaSampler;
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub struct LlamaModelService {
    model: Option<Arc<LlamaModel>>,
    model_info: Option<ModelInfo>,
    chat_template: Option<LlamaChatTemplate>,
    cancel_flag: Arc<AtomicBool>,
    conversation: Mutex<Vec<ChatMessage>>,
}

impl LlamaModelService {
    pub fn new() -> Result<Self> {
        // Initialize the process-wide llama backend once.
        let _ = shared_llama_backend()?;
        Ok(Self {
            model: None,
            model_info: None,
            chat_template: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            conversation: Mutex::new(Vec::new()),
        })
    }

    pub fn conversation(&self) -> Vec<ChatMessage> {
        self.conversation.lock().clone()
    }

    pub fn clear_conversation(&self) {
        self.conversation.lock().clear();
    }

    pub fn add_message(&self, message: ChatMessage) {
        self.conversation.lock().push(message);
    }

    pub fn format_conversation(&self) -> Result<String> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("no model loaded".into()))?;
        let template = self
            .chat_template
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("no chat template available".into()))?;
        let msgs = self.conversation.lock().clone();
        format_messages_with_model(model, template, &msgs)
    }
}

#[async_trait]
impl ModelService for LlamaModelService {
    async fn load(&mut self, path: &Path, options: LoadOptions) -> Result<()> {
        self.unload().await?;
        let backend = shared_llama_backend()?;
        let model_params = LlamaModelParams::default().with_n_gpu_layers(options.n_gpu_layers);
        let model = LlamaModel::load_from_file(backend, path, &model_params)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;

        let chat_template = model
            .chat_template(None)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let train_ctx = model.n_ctx_train();
        let n_ctx = if options.n_ctx == 0 {
            train_ctx.min(8192)
        } else {
            options.n_ctx.min(train_ctx.max(512))
        };

        self.model_info = Some(ModelInfo {
            path: path.to_path_buf(),
            filename,
            n_ctx,
            architecture: None,
        });
        self.chat_template = Some(chat_template);
        self.model = Some(Arc::new(model));
        Ok(())
    }

    async fn unload(&mut self) -> Result<()> {
        self.model = None;
        self.model_info = None;
        self.chat_template = None;
        Ok(())
    }

    fn is_loaded(&self) -> Option<ModelInfo> {
        self.model_info.clone()
    }

    fn generate_stream(
        &mut self,
        params: GenerateParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TokenEvent>> + Send>>> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("no model loaded".into()))?
            .clone();

        let n_ctx = self.model_info.as_ref().map(|m| m.n_ctx).unwrap_or(4096);
        let cancel = self.cancel_flag.clone();
        cancel.store(false, Ordering::SeqCst);

        let (tx, rx) = mpsc::channel(64);

        let prompt = params.prompt.clone();
        let max_tokens = params.max_tokens;
        let temperature = params.temperature;
        let top_p = params.top_p;
        let top_k = params.top_k;
        let repeat_penalty = params.repeat_penalty;
        let seed = params.seed;
        let stop_sequences = params.stop_sequences.clone();

        std::thread::spawn(move || {
            let result = run_generation(
                &model,
                &prompt,
                n_ctx,
                max_tokens,
                temperature,
                top_p,
                top_k,
                repeat_penalty,
                seed,
                &stop_sequences,
                &cancel,
                tx.clone(),
            );
            if let Err(e) = result {
                let _ = tx.blocking_send(Err(e));
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    fn format_prompt(&self, messages: &[ChatMessage]) -> Result<String> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("no model loaded".into()))?;
        let template = self
            .chat_template
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("no chat template available".into()))?;
        format_messages_with_model(model, template, messages)
    }
}

fn run_generation(
    model: &LlamaModel,
    prompt: &str,
    n_ctx: u32,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    repeat_penalty: f32,
    seed: i32,
    stop_sequences: &[String],
    cancel: &AtomicBool,
    tx: mpsc::Sender<Result<TokenEvent>>,
) -> Result<()> {
    let backend = shared_llama_backend()?;
    let n_ctx_nz =
        NonZeroU32::new(n_ctx).ok_or_else(|| AshkorixError::Model("invalid n_ctx".into()))?;
    // n_batch must fit the full prompt prefill; default (2048) is too small for RAG prompts
    // when n_ctx is larger (e.g. 4096 context with ~3k prompt tokens).
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx_nz))
        .with_n_batch(n_ctx);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;

    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;

    if tokens.is_empty() {
        return Err(AshkorixError::Model("prompt produced zero tokens".into()));
    }

    let max_prompt_tokens = n_ctx.saturating_sub(max_tokens).saturating_sub(16) as usize;
    let tokens = if tokens.len() > max_prompt_tokens {
        if max_prompt_tokens == 0 {
            return Err(AshkorixError::Model(format!(
                "prompt needs room in context (n_ctx={n_ctx}, max_tokens={max_tokens}); increase context_size or lower max_tokens"
            )));
        }
        tokens[tokens.len().saturating_sub(max_prompt_tokens)..].to_vec()
    } else {
        tokens
    };

    let batch_alloc = (tokens.len() + max_tokens as usize + 32)
        .min(n_ctx as usize)
        .max(512);
    let mut batch = LlamaBatch::new(batch_alloc, 1);
    batch
        .add_sequence(&tokens, 0, false)
        .map_err(|e| AshkorixError::Model(match e {
            llama_cpp_2::llama_batch::BatchAddError::InsufficientSpace(cap) => {
                format!(
                    "batch too small for prompt (capacity {cap}); increase context_size or shorten the conversation"
                )
            }
            other => other.to_string(),
        }))?;
    ctx.decode(&mut batch)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(temperature),
        LlamaSampler::top_p(top_p, 1),
        LlamaSampler::top_k(top_k),
        LlamaSampler::penalties(64, repeat_penalty, 0.0, 0.0),
        LlamaSampler::dist(if seed >= 0 {
            seed as u32
        } else {
            ggml_time_us() as u32
        }),
    ]);

    let mut generated = 0u32;
    let mut full_text = String::new();
    let mut n_cur = tokens.len() as i32;

    while generated < max_tokens {
        if cancel.load(Ordering::SeqCst) {
            return Err(AshkorixError::Cancelled);
        }

        if n_cur as u32 >= n_ctx {
            return Err(AshkorixError::Model(format!(
                "context full (n_ctx={n_ctx}); clear the conversation or increase context_size"
            )));
        }

        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            let _ = tx.blocking_send(Ok(TokenEvent {
                token: String::new(),
                finished: true,
                tokens_generated: generated,
            }));
            break;
        }

        let piece_bytes = token_to_piece_bytes(model, token)?;
        let piece = String::from_utf8_lossy(&piece_bytes).into_owned();
        full_text.push_str(&piece);
        generated += 1;

        if stop_sequences.iter().any(|s| full_text.ends_with(s)) {
            let _ = tx.blocking_send(Ok(TokenEvent {
                token: String::new(),
                finished: true,
                tokens_generated: generated,
            }));
            break;
        }

        let _ = tx.blocking_send(Ok(TokenEvent {
            token: piece,
            finished: false,
            tokens_generated: generated,
        }));

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;
        n_cur += 1;
        ctx.decode(&mut batch)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;
    }

    if generated >= max_tokens {
        let _ = tx.blocking_send(Ok(TokenEvent {
            token: String::new(),
            finished: true,
            tokens_generated: generated,
        }));
    }

    Ok(())
}

fn token_to_piece_bytes(model: &LlamaModel, token: LlamaToken) -> Result<Vec<u8>> {
    match model.token_to_piece_bytes(token, 8, false, None) {
        Err(TokenToStringError::InsufficientBufferSpace(i)) => {
            let size = usize::try_from(-i).map_err(|e| AshkorixError::Model(e.to_string()))?;
            model
                .token_to_piece_bytes(token, size, false, None)
                .map_err(|e| AshkorixError::Model(e.to_string()))
        }
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(AshkorixError::Model(e.to_string())),
    }
}

pub fn tokens_per_second(generated: u32, elapsed_ms: u128) -> f64 {
    if elapsed_ms == 0 {
        return 0.0;
    }
    generated as f64 / (elapsed_ms as f64 / 1000.0)
}
