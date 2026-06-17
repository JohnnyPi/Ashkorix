export interface CudaStatus {
  compiled: boolean;
  available: boolean;
  device_name: string | null;
}

export interface AshkorixConfig {
  data_dir: string;
  models_dir: string;
  embedding_model_path: string | null;
  reranker_model_path: string | null;
  default_retrieval_mode: string;
  log_level: string;
  local_only: boolean;
  generation: GenerationConfig;
  chunking: ChunkingConfig;
  retrieval: RetrievalConfig;
  memory: MemoryConfig;
}

export interface GenerationConfig {
  temperature: number;
  top_p: number;
  top_k: number;
  repeat_penalty: number;
  max_tokens: number;
  context_size: number;
  gpu_layers: number;
  threads: number;
  seed: number;
  stop_sequences: string[];
}

export interface ChunkingConfig {
  max_tokens: number;
  overlap_tokens: number;
}

export interface RetrievalConfig {
  max_query_variants: number;
  retrieval_context_budget_pct: number;
  candidate_pool_size: number;
}

export interface MemoryConfig {
  enabled: boolean;
  active_project: string;
  max_injected: number;
  min_confidence: number;
  min_importance: number;
  extraction_min_confidence: number;
}

export type MemoryType =
  | "user_preference"
  | "project_fact"
  | "decision"
  | "procedure";

export type MemoryStatus = "active" | "inactive" | "superseded" | "deleted";

export type CandidateStatus = "pending" | "approved" | "rejected" | "edited";

export interface Memory {
  id: string;
  memory_type: MemoryType;
  scope: string;
  title: string;
  content: string;
  importance: number;
  confidence: number;
  status: MemoryStatus;
  source_type: string | null;
  source_ref: string | null;
  created_at: string;
  updated_at: string;
  last_used_at: string | null;
  supersedes_id: string | null;
  metadata_json: string | null;
}

export interface MemoryCandidate {
  id: string;
  proposed_type: MemoryType;
  proposed_scope: string;
  proposed_title: string;
  proposed_content: string;
  importance: number;
  confidence: number;
  reason: string | null;
  source_type: string | null;
  source_ref: string | null;
  created_at: string;
  status: CandidateStatus;
}

export interface CreateMemoryInput {
  memory_type: MemoryType;
  scope: string;
  title: string;
  content: string;
  importance: number;
  confidence: number;
  source_type?: string | null;
  source_ref?: string | null;
  supersedes_id?: string | null;
  metadata_json?: string | null;
}

export interface UpdateMemoryInput {
  memory_type?: MemoryType;
  scope?: string;
  title?: string;
  content?: string;
  importance?: number;
  confidence?: number;
  status?: MemoryStatus;
  metadata_json?: string | null;
}

export interface EditCandidateInput {
  proposed_type?: MemoryType;
  proposed_scope?: string;
  proposed_title?: string;
  proposed_content?: string;
  importance?: number;
  confidence?: number;
}

export interface RetrievalFilters {
  document_ids: string[];
  file_types: string[];
  page_min: number | null;
  page_max: number | null;
  section_prefix: string | null;
  entity_match: string | null;
}

export const DEFAULT_RETRIEVAL_FILTERS: RetrievalFilters = {
  document_ids: [],
  file_types: [],
  page_min: null,
  page_max: null,
  section_prefix: null,
  entity_match: null,
};

export interface ModelFileInfo {
  path: string;
  filename: string;
  size_bytes: number;
}

export interface ModelInfo {
  path: string;
  filename: string;
  n_ctx: number;
  architecture: string | null;
}

export interface LoadOptions {
  n_ctx: number;
  n_gpu_layers: number;
  threads: number;
}

export const DEFAULT_LOAD_OPTIONS: LoadOptions = {
  n_ctx: 8192,
  n_gpu_layers: 0,
  threads: 4,
};

export interface ChatMessage {
  role: string;
  content: string;
}

export interface TokenPayload {
  token: string;
  finished: boolean;
  tokens_generated: number;
  citations?: Citation[];
  uncited_warning?: boolean;
  unsupported_claims?: UnsupportedClaim[];
}

export interface Document {
  id: string;
  content_hash: string;
  original_filename: string;
  file_path: string;
  file_type: string;
  collection_id: string;
  imported_at: string;
  title: string | null;
  author: string | null;
  chunk_count: number;
  import_status: string;
  extracted_text: string;
}

export interface ImportResult {
  document: Document | null;
  status: string;
  message: string;
}

export interface ImporterInfo {
  id: string;
  name: string;
  extensions: string[];
}

export interface Chunk {
  id: string;
  document_id: string;
  collection_id: string;
  text: string;
  token_count: number;
  source_filename: string;
  page_number: number | null;
  section_title: string | null;
  heading_path: string | null;
  chunk_index: number;
}

export interface IndexHealth {
  chunk_count: number;
  vector_count: number;
  lexical_count: number;
  indexed: boolean;
  embedding_loaded: boolean;
  embedding_model_path: string | null;
  message: string;
}

export interface RankedChunk {
  chunk: Chunk;
  score: number;
  source_type: string;
  source_number: number | null;
  rerank_score: number | null;
  expanded_context: string | null;
}

export interface Citation {
  source_number: number;
  document_id: string;
  original_filename: string;
  page_number: number | null;
  section_title: string | null;
  chunk_preview: string;
  score: number;
  collection_name: string;
}

export interface UnsupportedClaim {
  sentence: string;
  cited_source: number | null;
  reason: string;
}

export interface CorpusMapResult {
  themes: ThemeEntry[];
  entities: EntityFrequency[];
  sections: SectionEntry[];
  related_chunks: RankedChunk[];
}

export interface ThemeEntry {
  document_id: string;
  title: string;
  summary: string;
}

export interface EntityFrequency {
  value: string;
  count: number;
}

export interface SectionEntry {
  document_id: string;
  heading_path: string;
  summary: string | null;
}

export interface RagAnswer {
  text: string;
  citations: Citation[];
  dangling_citations: number[];
  uncited_warning: boolean;
  unsupported_claims: UnsupportedClaim[];
  retrieved_chunks: RankedChunk[];
  prompt: string;
  query_variants: string[];
  corpus_map: CorpusMapResult | null;
}

export interface DoctorCheck {
  name: string;
  path: string;
  ok: boolean;
  message: string;
}

export interface DoctorReport {
  local_only: boolean;
  checks: DoctorCheck[];
}

export interface ConversationExport {
  messages: ChatMessage[];
  exported_at: string;
}

export type RetrievalMode =
  | "fast"
  | "balanced"
  | "thorough"
  | "deep"
  | "corpus-map";

export type PageId = "chat" | "models" | "documents" | "search" | "memory" | "settings";
