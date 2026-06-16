# Ashkorix

Local-only desktop application for running GGUF models, importing documents, and cited RAG — no cloud APIs.

## Prerequisites (Windows)

Building the inference engine requires native tooling:

1. **Rust** (stable) — https://rustup.rs
2. **CMake** — https://cmake.org (or `winget install Kitware.CMake`)
3. **LLVM / libclang** (required by `llama-cpp-2` bindgen):
   ```powershell
   winget install LLVM.LLVM
   $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
   ```
4. **Visual Studio Build Tools** with the **Desktop development with C++** workload

The repo includes [`.cargo/config.toml`](.cargo/config.toml) setting `CMAKE_GENERATOR = "Visual Studio 17 2022"` because CMake 4.x may otherwise select a non-installed VS version.

Optional for GPU inference: CUDA toolkit (enable `cuda` feature on `llama-cpp-2` in `crates/ashkorix-core/Cargo.toml`).

## Build

```powershell
cd ashkorix
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"   # if not set permanently
cargo build -p ashkorix-core -p ashkorix-cli -p ashkorix-app
```

First build compiles llama.cpp from source and may take 15–30 minutes.

## Configuration

### Data directory

Runtime data lives in a top-level **`Data/`** folder:

| Context | Location |
|---------|----------|
| **Development** (`cargo run`) | [`../Data/`](../Data/) — set via `ASHKORIX_DATA_DIR` in [`.cargo/config.toml`](.cargo/config.toml) |
| **Portable / release** | `{exe_dir}/Data/` next to `Ashkorix.exe` or `ashkorix-cli.exe` |

On first run, Ashkorix creates `config.toml`, `models/`, `documents/`, `index/`, and `logs/` under the data root.

Override anytime with the `ASHKORIX_DATA_DIR` environment variable.

If you previously used `%LOCALAPPDATA%\Ashkorix\` or per-collection `Data/collections/`, copy chat models into `Data/models/` and re-import documents into the shared pool.

| Setting | Purpose |
|---------|---------|
| `models_dir` | Directory scanned for `.gguf` chat models (default: `Data/models`) |
| `embedding_model_path` | Path to a GGUF **embedding** model (e.g. `Data/models/embeddings/...`) |
| `data_dir` | SQLite DB, documents, and global index |

Example `config.toml` (paths relative to data root or absolute):

```toml
embedding_model_path = "models/embeddings/nomic-embed-text-v1.5.Q8_0.gguf"
```

## CLI quick start

```powershell
# Health check
cargo run -p ashkorix-cli -- doctor

# List discovered GGUF files
cargo run -p ashkorix-cli -- models list

# Chat with a local model (real llama.cpp inference)
cargo run -p ashkorix-cli -- chat --model "..\Data\models\your-model.gguf"

# Import + index + ask (requires embedding_model_path)
cargo run -p ashkorix-cli -- import .\docs\file.txt
cargo run -p ashkorix-cli -- index build
cargo run -p ashkorix-cli -- retrieve --query "What is this about?"
cargo run -p ashkorix-cli -- ask --query "Summarize the document" --model "..\Data\models\your-model.gguf"
```

## Architecture

- `ashkorix-core` — engine (llama.cpp GGUF, importers, chunking, tantivy + vector index, RAG)
- `ashkorix-cli` — headless harness
- `ashkorix-app` — Tauri 2 desktop shell (React + Vite UI)

All documents share one **knowledge pool** (`Data/documents/` + `Data/index/`). There are no user-facing collections.

## Desktop app

```powershell
cd ashkorix/crates/ashkorix-app
npm install
npm run tauri dev
```

Header navigation: **Chat** | **Models** | **Documents** | **Search** | **Settings**

- **Chat** — streaming chat; toggle **Use knowledge base** for cited RAG (`ask`)
- **Documents** — import, list, delete, build/rebuild index
- **Search** — retrieval inspector (ranked chunks only, no generation)
- **Settings** — generation params, embedding model path, doctor diagnostics

Use **Open Data folder** in the header to reveal `Data/` in Explorer.

Production build:

```powershell
npm run tauri build
```

Runtime data resolves to `{exe_dir}/Data/` beside the installed app (see Configuration above).

All inference uses **real** `llama-cpp-2` bindings. There is no mock model service. Embedding and chat failures surface as errors rather than silent fallbacks.

## Troubleshooting

| Error | Fix |
|-------|-----|
| `Unable to find libclang` | Install LLVM; set `LIBCLANG_PATH` |
| `embedding model not loaded` | Set `embedding_model_path` in config to a GGUF embedding model |
| `model returned empty embedding` | Use a dedicated embedding GGUF, not a chat model |
| `BackendAlreadyInitialized` | Should not occur; backend is shared via singleton |
