import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_LOAD_OPTIONS } from "./types";
import type {
  AshkorixConfig,
  ConversationExport,
  DoctorReport,
  Document,
  GenerationConfig,
  ImportResult,
  ImporterInfo,
  IndexHealth,
  ModelFileInfo,
  ModelInfo,
  LoadOptions,
  RagAnswer,
  RankedChunk,
  RetrievalFilters,
} from "./types";
import { DEFAULT_RETRIEVAL_FILTERS } from "./types";

export const api = {
  getVersion: () => invoke<string>("get_version"),
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
};

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
