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

Optional for GPU inference: install the [CUDA toolkit](https://developer.nvidia.com/cuda-downloads) (CUDA is enabled automatically in desktop/CLI builds; set `gpu_layers = 0` in config for auto-detect, or a positive value to override).

## Build

```powershell
cd ashkorix
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"   # if not set permanently
.\scripts\build.ps1
```

On Windows with CUDA, use `.\scripts\build.ps1` so `CUDA_PATH` matches your Visual Studio CUDA integration (v13.3 vs v12.5). **Do not use plain `cargo build` on Windows with GPU** — if your system `CUDA_PATH` (often v12.5) differs from the CUDA version wired into Visual Studio (often v13.3), llama.cpp CMake will fail with `The CUDA Toolkit directory '' does not exist`.

`build.ps1` builds **all three** binaries: `ashkorix-core`, `ashkorix-app`, and `ashkorix-cli`.

First build compiles llama.cpp from source and may take 15–30 minutes.

`build-cuda.ps1` is an alias for `build.ps1`.

If CMake reports `No CUDA toolset found`, install the CUDA Visual Studio integration (see Troubleshooting).

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

On Windows with CUDA, use the wrapper so the CLI gets the same toolkit path as the desktop app:

```powershell
# Health check
.\scripts\run-cli.ps1 -- doctor

# List discovered GGUF files
.\scripts\run-cli.ps1 -- models list

# Chat with a local model (real llama.cpp inference)
.\scripts\run-cli.ps1 -- chat --model "..\Data\models\your-model.gguf"

# Import + index + ask (requires embedding_model_path)
.\scripts\run-cli.ps1 -- import .\docs\file.txt
.\scripts\run-cli.ps1 -- index build
.\scripts\run-cli.ps1 -- retrieve --query "What is this about?"
.\scripts\run-cli.ps1 -- ask --query "Summarize the document" --model "..\Data\models\your-model.gguf"
```

After `.\scripts\build.ps1`, you can also run `target\debug\ashkorix.exe` directly. Plain `cargo run -p ashkorix-cli` without the CUDA scripts may fail to compile or link llama.cpp on Windows.

## Architecture

- `ashkorix-core` — engine (llama.cpp GGUF, importers, chunking, tantivy + vector index, RAG)
- `ashkorix-cli` — headless harness
- `ashkorix-app` — Tauri 2 desktop shell (React + Vite UI)

All documents share one **knowledge pool** (`Data/documents/` + `Data/index/`). There are no user-facing collections.

## Desktop app

```powershell
cd ashkorix/crates/ashkorix-app
npm install
npm run tauri dev          # CPU-only if llama.cpp already built
.\scripts\dev-cuda.ps1     # from ashkorix/ — GPU builds with aligned CUDA_PATH
```

Header navigation: **Chat** | **Models** | **Documents** | **Search** | **Memory** | **Settings**

- **Chat** — streaming chat; toggle **Use knowledge base** for cited RAG (`ask`); **Extract memories** proposes durable facts to the inbox
- **Documents** — import, list, delete, build/rebuild index
- **Search** — retrieval inspector (ranked chunks only, no generation)
- **Memory** — active memories and candidate inbox (approve / reject / edit)
- **Settings** — generation params, embedding model path, memory config, doctor diagnostics

Use **Open Data folder** in the header to reveal `Data/` in Explorer.

Production build:

```powershell
npm run tauri build
```

Runtime data resolves to `{exe_dir}/Data/` beside the installed app (see Configuration above).

All inference uses **real** `llama-cpp-2` bindings. There is no mock model service. Embedding and chat failures surface as errors rather than silent fallbacks.

## Memory

Ashkorix stores durable user/project memory in SQLite (`Data/ashkorix.db`), separate from the document knowledge pool.

### Memory types

| Type | Purpose |
|------|---------|
| `user_preference` | How the user likes responses shaped |
| `project_fact` | Stable facts about a project |
| `decision` | Choices the user made (prevents backtracking) |
| `procedure` | Reusable workflow preferences |

### Scopes

Memories are scoped to avoid cross-project pollution:

- `global` — cross-project preferences and procedures
- `project:{name}` — project facts and decisions (default project: `ashkorix`)
- `conversation:{session_id}` — session-bound context

### Inbox workflow

Memories are **never written silently**. After a chat session, click **Extract memories** on the Chat page (or use CLI). Candidates land in the **Memory inbox** for approve / reject / edit before becoming active.

### Config (`[memory]` in `config.toml`)

```toml
[memory]
enabled = true
active_project = "ashkorix"
max_injected = 8
min_confidence = 0.75
min_importance = 0.0
extraction_min_confidence = 0.75
```

Retrieval ranks memories by scope match, type relevance, importance, embedding similarity, and recency. Top 3–8 memories are injected into chat and RAG prompts as a plain-text block.

### CLI

```powershell
cargo run -p ashkorix-cli -- memory list
cargo run -p ashkorix-cli -- memory list --scope project:ashkorix
cargo run -p ashkorix-cli -- memory inbox
cargo run -p ashkorix-cli -- memory approve <id>
cargo run -p ashkorix-cli -- memory reject <id>
cargo run -p ashkorix-cli -- memory extract --model "..\Data\models\your-model.gguf"
cargo run -p ashkorix-cli -- memory add --memory-type project_fact --scope project:ashkorix --title "Title" --content "Content"
```

## Troubleshooting

| Error | Fix |
|-------|-----|
| `Unable to find libclang` | Install LLVM; set `LIBCLANG_PATH` |
| `embedding model not loaded` | Set `embedding_model_path` in config to a GGUF embedding model |
| `model returned empty embedding` | Use a dedicated embedding GGUF, not a chat model |
| `BackendAlreadyInitialized` | Should not occur; backend is shared via singleton |
| `The CUDA Toolkit directory '' does not exist` | Your `CUDA_PATH` (often v12.5) does not match the CUDA version wired into Visual Studio (often v13.3). Run `.\scripts\build.ps1` or set `CUDA_PATH` to the matching toolkit, e.g. `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3`. |
| `No CUDA toolset found` / CUDA build fails | Run the CUDA installer’s Visual Studio integration (or copy `CUDA v12.5.*` from `CUDA\\extras\\visual_studio_integration\\MSBuildExtensions` into VS 2022 `BuildCustomizations` as admin). Build from a **VS 2022 x64 Native Tools** prompt. |
| Header shows `CUDA OFF` with an RTX GPU | Rebuild the app after a successful CUDA build (`cargo build` with the `cuda` feature enabled on `ashkorix-core`). |
