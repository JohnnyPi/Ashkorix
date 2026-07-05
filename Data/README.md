# Ashkorix Data

Runtime data for Ashkorix lives here during development. In a portable install, the same layout sits beside the executable as `{exe_dir}/Data/`.

## Layout

| Path | Purpose |
|------|---------|
| `config.toml` | App settings (created on first run) |
| `config.toml.example` | Starter template — copy to `config.toml` and edit paths |
| `ashkorix.db` | Document and chunk metadata (SQLite) |
| `documents/` | Imported source files (shared knowledge pool) |
| `index/` | Global search index (`tantivy/` + `vectors.usearch`) |
| `models/` | Chat GGUF models (scanned recursively) |
| `models/embeddings/` | Optional location for embedding GGUF models |
| `logs/` | Daily rolling log files |

Legacy `collections/` folders from older builds are ignored; re-import documents if you migrated from per-collection storage.

## First-time setup

1. Copy `config.toml.example` to `config.toml`.
2. Adjust model paths if you use embedding or reranker GGUF files.
3. Run `cargo run -p ashkorix-cli -- doctor` from `ashkorix/` to verify discovery.

## Adding models

1. Copy `.gguf` chat models into `models/` (any subfolder is fine).
2. Set `embedding_model_path` in `config.toml` to an embedding GGUF (e.g. under `models/embeddings/`).
3. Run `cargo run -p ashkorix-cli -- doctor` from `ashkorix/` to verify discovery.

## RAG workflow

1. Import files via the **Documents** page or `cargo run -p ashkorix-cli -- import <files>`
2. Build the index: **Documents → Build index** or `cargo run -p ashkorix-cli -- index build`
3. Search ranked chunks on the **Search** tab or `retrieve --query "..."`
4. Chat with **Use knowledge base** enabled or `ask --query "..."`

## Overrides

- **Development:** `ASHKORIX_DATA_DIR` is set in `ashkorix/.cargo/config.toml` to point here.
- **Release / portable:** No env var; Ashkorix uses `Data/` next to the `.exe`.

If you previously used `%LOCALAPPDATA%\Ashkorix\`, copy its contents into this folder once.
