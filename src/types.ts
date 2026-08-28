export interface FileRecord {
  id: number;
  session_id: number;
  path: string;
  file_name: string;
  extension: string | null;
  size_bytes: number;
  created_at_ms: number | null;
  modified_at_ms: number | null;
  accessed_at_ms: number | null;
  is_empty: boolean;
  hash: string | null;
  hash_algorithm: string | null;
  hash_status: string;
  scan_status: string;
  error: string | null;
}

export interface ScanSession {
  id: number;
  roots: string[];
  started_at_ms: number;
  finished_at_ms: number | null;
  status: "running" | "completed" | "cancelled" | "failed";
}

export interface ScanSummary {
  session: ScanSession;
  files_scanned: number;
  total_size_bytes: number;
  empty_files: number;
  inaccessible_files: number;
  error_files: number;
  duplicate_groups: number;
  duplicate_files: number;
  potential_reclaimable_bytes: number;
}

export interface DuplicateGroup {
  id: number;
  session_id: number;
  hash: string;
  size_bytes: number;
  members: FileRecord[];
  recommended_keep_file_id: number;
}

/** Reclaimable space = total minus one kept copy. */
export function groupReclaimable(g: DuplicateGroup): number {
  return g.size_bytes * Math.max(0, g.members.length - 1);
}

export interface ScanProgress {
  session_id: number;
  phase: string;
  files_discovered: number;
  files_processed: number;
  candidates_total: number;
  candidates_hashed: number;
  errors: number;
  current_path: string;
}

export type ScanFinished = {
  session_id: number;
  status: "completed" | "cancelled" | "failed";
  summary?: ScanSummary;
  error?: string;
};

export interface QuarantineItem {
  id: number;
  operation_id: number;
  original_path: string;
  quarantine_path: string;
  file_name: string;
  size_bytes: number;
  hash: string | null;
  quarantined_at_ms: number;
  status: "stored" | "restored" | "deleted";
}

export interface BatchReport {
  operation_id: number;
  batch_id: string;
  succeeded: number;
  failed: number;
  failures: { path: string; error: string }[];
}

export interface RestorePreview {
  item_id: number;
  original_path: string;
  quarantine_path: string;
  file_exists_at_original: boolean;
}

export interface FileOperation {
  id: number;
  batch_id: string;
  kind: "quarantine" | "restore" | "delete_permanent";
  status: string;
  started_at_ms: number;
  finished_at_ms: number | null;
  detail: string | null;
}

export interface OperationItem {
  id: number;
  operation_id: number;
  source: string | null;
  destination: string | null;
  file_id: number | null;
  status: string;
  error: string | null;
}

export interface AppSettings {
  default_scan_location: string | null;
  quarantine_retention_days: number;
  max_hash_concurrency: number;
}
