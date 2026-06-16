use ashkorix_core::app::AppState;
use ashkorix_core::config::discover_gguf_models;
use ashkorix_core::rag::types::{RetrievalFilters, RetrievalMode};
use ashkorix_core::traits::model::ChatMessage;
use ashkorix_core::traits::model::LoadOptions;
use ashkorix_core::traits::ModelService;
use clap::{Parser, Subcommand};
use futures::StreamExt;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ashkorix", about = "Local-only GGUF RAG runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Models {
        #[command(subcommand)]
        action: ModelsAction,
    },
    Doctor,
    Chat {
        #[arg(long)]
        model: PathBuf,
    },
    Generate {
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        prompt: String,
    },
    Import {
        files: Vec<PathBuf>,
    },
    Documents {
        #[command(subcommand)]
        action: DocumentsAction,
    },
    Index {
        #[command(subcommand)]
        action: IndexAction,
    },
    Retrieve {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "balanced")]
        mode: String,
        #[arg(long)]
        filter_doc: Option<String>,
        #[arg(long)]
        filter_section: Option<String>,
    },
    Ask {
        #[arg(long)]
        query: String,
        #[arg(long)]
        model: Option<PathBuf>,
        #[arg(long, default_value = "balanced")]
        mode: String,
        #[arg(long)]
        filter_doc: Option<String>,
        #[arg(long)]
        filter_section: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    Show,
}

#[derive(Subcommand)]
enum ModelsAction {
    List,
}

#[derive(Subcommand)]
enum DocumentsAction {
    List,
    Delete { id: String },
}

#[derive(Subcommand)]
enum IndexAction {
    Build,
    Rebuild,
    Health,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut state = AppState::new()?;

    match cli.command {
        Commands::Config { action: ConfigAction::Show } => {
            println!("{}", toml::to_string_pretty(&state.config)?);
        }
        Commands::Models { action: ModelsAction::List } => {
            for m in discover_gguf_models(&state.config.models_dir)? {
                println!("{} ({} bytes)", m.filename, m.size_bytes);
            }
        }
        Commands::Doctor => {
            let report = state.doctor();
            println!("local_only: {}", report.local_only);
            for c in report.checks {
                println!(
                    "[{}] {} ({}) - {}",
                    if c.ok { "OK" } else { "FAIL" },
                    c.name,
                    c.path,
                    c.message
                );
            }
        }
        Commands::Chat { model } => {
            state.load_model(model, LoadOptions::default()).await?;
            println!("Chat started. Type 'exit' to quit.");
            let stdin = tokio::io::stdin();
            let mut reader = tokio::io::BufReader::new(stdin);
            use tokio::io::AsyncBufReadExt;
            loop {
                print!("> ");
                let mut line = String::new();
                reader.read_line(&mut line).await?;
                let line = line.trim();
                if line == "exit" {
                    break;
                }
                let mut stream = state
                    .chat_stream(line.to_string(), state.config.generation.clone())
                    .await?;
                let mut assistant = String::new();
                while let Some(event) = stream.next().await {
                    let event = event?;
                    assistant.push_str(&event.token);
                    print!("{}", event.token);
                    if event.finished {
                        break;
                    }
                }
                println!();
                state.model.lock().await.add_message(ChatMessage {
                    role: "assistant".into(),
                    content: assistant,
                });
            }
        }
        Commands::Generate { model, prompt } => {
            state.load_model(model, LoadOptions::default()).await?;
            let mut stream = state
                .model
                .lock()
                .await
                .generate_stream(ashkorix_core::traits::model::GenerateParams {
                    prompt,
                    temperature: state.config.generation.temperature,
                    top_p: state.config.generation.top_p,
                    top_k: state.config.generation.top_k,
                    repeat_penalty: state.config.generation.repeat_penalty,
                    max_tokens: state.config.generation.max_tokens,
                    seed: state.config.generation.seed,
                    stop_sequences: state.config.generation.stop_sequences.clone(),
                })?;
            while let Some(event) = stream.next().await {
                let event = event?;
                print!("{}", event.token);
                if event.finished {
                    break;
                }
            }
            println!();
        }
        Commands::Import { files } => {
            let results = state.import_files(files).await?;
            for r in results {
                println!("{}: {:?}", r.message, r.status);
            }
        }
        Commands::Documents { action } => match action {
            DocumentsAction::List => {
                for doc in state.list_documents()? {
                    println!(
                        "{} {} [{} chunks] {:?}",
                        doc.id.0, doc.original_filename, doc.chunk_count, doc.import_status
                    );
                }
            }
            DocumentsAction::Delete { id } => {
                state.delete_document(&id)?;
                println!("deleted {id}");
            }
        },
        Commands::Index { action } => match action {
            IndexAction::Build => {
                let health = state.build_index().await?;
                println!("{health:?}");
            }
            IndexAction::Rebuild => {
                let health = state.rebuild_index().await?;
                println!("{health:?}");
            }
            IndexAction::Health => {
                let health = state.index_health()?;
                println!("{health:?}");
            }
        },
        Commands::Retrieve {
            query,
            mode,
            filter_doc,
            filter_section,
        } => {
            let mut filters = RetrievalFilters::default();
            if let Some(doc) = filter_doc {
                filters.document_ids.push(doc);
            }
            filters.section_prefix = filter_section;
            let chunks = state
                .retrieve(&query, RetrievalMode::from_str(&mode), vec![], filters)
                .await?;
            for c in chunks {
                println!(
                    "[{}] {} {} - score {:.4}{}",
                    c.source_type,
                    c.chunk.source_filename,
                    c.chunk.id.0,
                    c.score,
                    c.rerank_score
                        .map(|s| format!(" rerank={s:.4}"))
                        .unwrap_or_default()
                );
                if let Some(ref path) = c.chunk.heading_path {
                    println!("  path: {path}");
                }
            }
        }
        Commands::Ask {
            query,
            model,
            mode,
            filter_doc,
            filter_section,
        } => {
            if let Some(model_path) = model {
                state.load_model(model_path, LoadOptions::default()).await?;
            }
            let mut filters = RetrievalFilters::default();
            if let Some(doc) = filter_doc {
                filters.document_ids.push(doc);
            }
            filters.section_prefix = filter_section;
            let answer = state
                .ask(
                    &query,
                    RetrievalMode::from_str(&mode),
                    state.config.generation.clone(),
                    vec![],
                    filters,
                )
                .await?;
            println!("{}", answer.text);
            if !answer.query_variants.is_empty() {
                println!("[query variants] {}", answer.query_variants.join(" | "));
            }
            if answer.uncited_warning {
                println!("[warning] answer has no citations");
            }
            for d in answer.dangling_citations {
                println!("[warning] dangling citation: [Source {d}]");
            }
            for u in answer.unsupported_claims {
                println!("[verify] unsupported: {}", u.reason);
            }
            if let Some(map) = answer.corpus_map {
                println!("[corpus map] {} themes, {} entities", map.themes.len(), map.entities.len());
            }
        }
    }

    Ok(())
}
