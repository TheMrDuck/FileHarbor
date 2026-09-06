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
  default_backup_destination: string | null;
  default_verification_mode: VerificationMode;
  max_backup_concurrency: number;
  default_backup_schedule: BackupSchedule;
  default_retention_policy: RetentionPolicy;
}

// Backup types
export type BackupSchedule = "manual" | "daily" | "weekly" | "on_external_drive_connected";
export type RetentionPolicy = "keep_all" | `keep_last_${number}`;
export type VerificationMode = "standard" | "deep";
export type BackupSetStatus = "active" | "outdated" | "at_risk" | "paused" | "disabled";
export type BackupRunStatus = "running" | "verifying" | "committing" | "completed" | "failed" | "partial" | "cancelled";
export type BackupSnapshotStatus = "verified" | "unverified" | "corrupted";
export type BackupFileStatus = "verified" | "pending" | "failed" | "skipped" | "source_missing" | "unstable";
export type RestoreConflictPolicy = "keep_existing" | "restore_with_new_name" | "cancel";

export interface BackupSet {
  id: number;
  name: string;
  source_path: string;
  destination_id: number;
  schedule: BackupSchedule;
  retention_policy: RetentionPolicy;
  verification_mode: VerificationMode;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface BackupDestination {
  id: number;
  destination_id: string;
  path: string;
  label: string | null;
  filesystem_id: string | null;
  created_at_ms: number;
  last_seen_at_ms: number | null;
}

export interface BackupRun {
  id: number;
  backup_set_id: number;
  snapshot_id: number | null;
  status: BackupRunStatus;
  started_at_ms: number;
  finished_at_ms: number | null;
  files_total: number;
  files_new: number;
  files_changed: number;
  files_unchanged: number;
  files_deleted: number;
  files_failed: number;
  bytes_transferred: number;
  bytes_verified: number;
  error_summary: string | null;
}

export interface BackupSnapshot {
  id: number;
  backup_set_id: number;
  run_id: number;
  label: string;
  status: BackupSnapshotStatus;
  created_at_ms: number;
  files_count: number;
  total_size_bytes: number;
}

export interface BackupPlan {
  new_files: PlannedFile[];
  changed_files: PlannedFile[];
  unchanged_files: PlannedFile[];
  deleted_files: PlannedFile[];
  unavailable_files: PlannedFile[];
  total_files: number;
  total_new_size: number;
  total_changed_size: number;
  estimated_transfer_bytes: number;
}

export interface PlannedFile {
  source_path: string;
  relative_path: string;
  size_bytes: number;
  sha256: string | null;
  modified_at_ms: number | null;
}

export interface BackupProgress {
  run_id: number;
  status: BackupRunStatus;
  current_file: string | null;
  files_processed: number;
  files_total: number;
  bytes_transferred: number;
  bytes_total: number;
  bytes_verified: number;
  files_verified: number;
  files_failed: number;
  speed_bytes_per_sec: number;
  eta_seconds: number | null;
}

export interface BackupHealth {
  total_files: number;
  protected_files: number;
  outdated_files: number;
  never_backed_up_files: number;
  protection_percentage: number;
  sets: BackupSetHealth[];
}

export interface BackupSetHealth {
  backup_set_id: number;
  name: string;
  status: BackupSetStatus;
  last_verified_at_ms: number | null;
  protected_files: number;
  outdated_files: number;
  never_backed_up_files: number;
  protection_percentage: number;
}

export interface DestinationValidation {
  valid: boolean;
  destination_id: string | null;
  errors: string[];
  warnings: string[];
  available_space_bytes: number | null;
  estimated_required_bytes: number | null;
}

export interface BackupRunSummary {
  run_id: number;
  snapshot_id: number;
  status: BackupRunStatus;
  files_total: number;
  files_new: number;
  files_changed: number;
  files_unchanged: number;
  files_deleted: number;
  files_failed: number;
  bytes_transferred: number;
  bytes_verified: number;
  duration_ms: number;
}

export interface RestoreResult {
  requested: number;
  restored: number;
  verified: number;
  failed: number;
  skipped: number;
  conflicts_resolved: number;
  errors: { relative_path: string; error: string }[];
}

export interface RestorePreview {
  total_files: number;
  existing_files: number;
  missing_files: number;
  conflicts: RestoreConflict[];
}

export interface RestoreConflict {
  relative_path: string;
  existing_size: number;
  existing_hash: string | null;
  backup_size: number;
  backup_hash: string;
}

export interface RestoreFromBackupRequest {
  snapshot_id: number;
  destination_root: string;
  conflicts: { relative_path: string; policy: RestoreConflictPolicy }[];
}

export interface FileBackupStatus {
  file_id: number;
  status: "protected" | "outdated" | "never_backed_up" | "backup_missing" | "source_missing" | "unstable" | "error";
  last_verified_at_ms: number | null;
  backup_set_id: number | null;
  backup_set_name: string | null;
  snapshot_id: number | null;
  snapshot_label: string | null;
}

export interface GetFileBackupStatusRequest {
  session_id: number;
  file_ids: number[];
}
