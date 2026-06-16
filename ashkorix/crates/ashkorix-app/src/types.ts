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

export type PageId = "chat" | "models" | "documents" | "search" | "settings";
