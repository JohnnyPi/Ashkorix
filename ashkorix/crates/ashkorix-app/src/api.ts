import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_LOAD_OPTIONS } from "./types";
import type {
  AshkorixConfig,
  ConversationExport,
  CudaStatus,
  DoctorReport,
  Document,
  EditCandidateInput,
  GenerationConfig,
  ImportResult,
  ImporterInfo,
  IndexHealth,
  CreateMemoryInput,
  Memory,
  MemoryCandidate,
  ModelFileInfo,
  ModelInfo,
  LoadOptions,
  RagAnswer,
  RankedChunk,
  RetrievalFilters,
  UpdateMemoryInput,
} from "./types";
import { DEFAULT_RETRIEVAL_FILTERS } from "./types";

export const api = {
  getVersion: () => invoke<string>("get_version"),
  getCudaStatus: () => invoke<CudaStatus>("get_cuda_status"),
  getConfig: () => invoke<AshkorixConfig>("get_config"),
  updateConfig: (config: AshkorixConfig) => invoke<void>("update_config", { config }),
  openDataFolder: () => invoke<void>("open_data_folder"),

  listModels: () => invoke<ModelFileInfo[]>("list_models"),
  loadModel: (path: string, options: Partial<LoadOptions> = {}) =>
    invoke<void>("load_model", {
      path,
      options: { ...DEFAULT_LOAD_OPTIONS, ...options },
    }),
  unloadModel: () => invoke<void>("unload_model"),
  getModelInfo: () => invoke<ModelInfo | null>("get_model_info"),

  chatStreamStart: (message: string) => invoke<void>("chat_stream_start", { message }),
  ragStreamStart: (question: string, mode: string) =>
    invoke<void>("rag_stream_start", { question, mode }),
  cancelGeneration: () => invoke<void>("cancel_generation"),
  clearConversation: () => invoke<void>("clear_conversation"),
  saveConversation: () => invoke<ConversationExport>("save_conversation"),
  getGenerationSettings: () => invoke<GenerationConfig>("get_generation_settings"),
  setGenerationSettings: (settings: GenerationConfig) =>
    invoke<void>("set_generation_settings", { settings }),

  listImporters: () => invoke<ImporterInfo[]>("list_importers"),
  importFiles: (paths: string[]) => invoke<ImportResult[]>("import_files", { paths }),
  listDocuments: () => invoke<Document[]>("list_documents"),
  deleteDocument: (id: string) => invoke<void>("delete_document", { id }),

  buildIndex: () => invoke<IndexHealth>("build_index"),
  rebuildIndex: () => invoke<IndexHealth>("rebuild_index"),
  indexHealth: () => invoke<IndexHealth>("index_health"),

  retrieve: (
    query: string,
    mode: string,
    exclude: string[] = [],
    filters: RetrievalFilters = DEFAULT_RETRIEVAL_FILTERS,
  ) => invoke<RankedChunk[]>("retrieve", { query, mode, exclude, filters }),
  ask: (
    question: string,
    mode: string,
    exclude: string[] = [],
    filters: RetrievalFilters = DEFAULT_RETRIEVAL_FILTERS,
  ) => invoke<RagAnswer>("ask", { question, mode, exclude, filters }),

  doctor: () => invoke<DoctorReport>("doctor"),

  listMemories: (scopeFilter?: string) =>
    invoke<Memory[]>("list_memories", { scopeFilter: scopeFilter ?? null }),
  searchMemories: (query: string, limit?: number) =>
    invoke<Memory[]>("search_memories", { query, limit: limit ?? null }),
  listMemoryCandidates: () => invoke<MemoryCandidate[]>("list_memory_candidates"),
  approveMemoryCandidate: (id: string) =>
    invoke<Memory>("approve_memory_candidate", { id }),
  rejectMemoryCandidate: (id: string) => invoke<void>("reject_memory_candidate", { id }),
  editAndApproveCandidate: (id: string, edit: EditCandidateInput) =>
    invoke<Memory>("edit_and_approve_candidate", { id, edit }),
  createMemory: (input: CreateMemoryInput) => invoke<Memory>("create_memory", { input }),
  updateMemory: (id: string, input: UpdateMemoryInput) =>
    invoke<Memory>("update_memory", { id, input }),
  deactivateMemory: (id: string) => invoke<void>("deactivate_memory", { id }),
  extractMemoryCandidates: () => invoke<MemoryCandidate[]>("extract_memory_candidates"),
  getLastInjectedMemories: () => invoke<Memory[]>("get_last_injected_memories"),
};

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
