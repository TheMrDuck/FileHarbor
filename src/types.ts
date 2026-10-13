export type CopyRole = "working" | "backup" | "quarantine" | "unknown";

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
  copy_role?: CopyRole;
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

/** Reclaimable space = total minus kept copy, excluding protected backup copies (Section 4 & 97). */
export function groupReclaimable(g: DuplicateGroup): number {
  const workingCopies = g.members.filter((m) => m.copy_role !== "backup").length;
  return g.size_bytes * Math.max(0, workingCopies - 1);
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

export interface QuarantineRestorePreview {
  item_id: number;
  original_path: string;
  quarantine_path: string;
  file_exists_at_original: boolean;
}

export type RestorePreview = QuarantineRestorePreview;

export interface FileOperation {
  id: number;
  batch_id: string;
  kind:
    | "quarantine"
    | "restore"
    | "delete_permanent"
    | "backup"
    | "backup_verify"
    | "backup_restore"
    | "backup_cancel";
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
  privacy_shield_enabled: boolean;
  default_backup_destination: string | null;
  default_verification_mode: VerificationMode;
  max_backup_concurrency: number;
  default_backup_schedule: BackupSchedule;
  default_retention_policy: RetentionPolicy;
}

export interface PrivacyAuditResult {
  network_status: string;
  outbound_bytes: number;
  privacy_shield_enabled: boolean;
  shielded_directories: string[];
  shielded_extensions: string[];
  storage_guarantee: string;
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

export interface BackupRestorePreview {
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

export interface TestScenarioInfo {
  name: string;
  scenario_type: string;
  description: string;
  default_files: number;
}

export interface GenerateTestDataRequest {
  scenario: string;
  output_dir: string;
  files?: number;
  seed?: number;
  duplicate_groups?: number;
  empty_files?: number;
  unicode_files?: number;
  large_files?: number;
  nested_depth?: number;
  same_size_diff_content?: number;
  allow_dangerous?: boolean;
}

export interface TestGenerationProgress {
  directories_created: number;
  files_created: number;
  bytes_written: number;
  duplicate_groups_created: number;
}

export interface TestManifestSummary {
  files: number;
  directories: number;
  empty_files: number;
  duplicate_groups: number;
  duplicate_files: number;
  unicode_files: number;
  large_files: number;
  nested_files: number;
  same_size_different_content_pairs: number;
  symlinks: number;
  broken_symlinks: number;
  backup_like_copies: number;
}

export interface SizeMismatch {
  path: string;
  expected: number;
  actual: number;
}

export interface HashMismatch {
  path: string;
  expected: string;
  actual: string;
}

export interface DuplicateGroupResult {
  group_id: string;
  expected_count: number;
  actual_count: number;
  members: string[];
  all_hashes_match: boolean;
}

export interface EmptyFileResult {
  path: string;
  expected_empty: boolean;
  actual_empty: boolean;
}

export interface VerificationResult {
  expected_files: number;
  found_files: number;
  missing_files: string[];
  unexpected_files: string[];
  size_mismatches: SizeMismatch[];
  hash_mismatches: HashMismatch[];
  duplicate_group_results: DuplicateGroupResult[];
  empty_file_results: EmptyFileResult[];
  missing_expected: number;
  missing_actual: number;
}

export interface TrackedTestDataset {
  id: number;
  scenario: string;
  path: string;
  total_files: number;
  total_bytes: number;
  created_at_ms: number;
}

// ----------------------------------------------------------------------
// File Health & Recovery Types
// ----------------------------------------------------------------------

export type MagicByteStatus =
  | { Valid: { detected_format: string } }
  | { Mismatch: { detected_format: string; expected_format: string } }
  | { CorruptedHeader: { details: string } }
  | "NotApplicable";

export interface NullRunStats {
  total_bytes_analyzed: number;
  total_null_bytes: number;
  null_percentage: number;
  longest_null_run_bytes: number;
  sector_sized_null_blocks: number;
}

export interface FileHealthReport {
  file_path: string;
  file_name: string;
  file_size: number;
  sha256: string;
  magic_status: MagicByteStatus;
  null_stats: NullRunStats;
  entropy: number;
  health_score: number;
  issues: string[];
}

export type HealthRecommendation =
  | "KeepA"
  | "KeepB"
  | "BothHealthy"
  | "BothCorrupted"
  | "Inconclusive";

export interface RecoveryPairComparison {
  pair_id: string;
  base_name: string;
  candidate_a: FileHealthReport;
  candidate_b: FileHealthReport;
  recommendation: HealthRecommendation;
  explanation: string;
}

// ----------------------------------------------------------------------
// Downloads & Atom Files Triage Types
// ----------------------------------------------------------------------

export type TriageCategory =
  | "Documents"
  | "Images"
  | "AudioVideo"
  | "Archives"
  | "Installers"
  | "IncompleteDownloads"
  | "Code"
  | "AtomOrOther";

export interface TriageItem {
  file_path: string;
  file_name: string;
  extension: string;
  file_size: number;
  modified_timestamp: number;
  category: TriageCategory;
  is_stale_installer: boolean;
  is_incomplete_download: boolean;
  suggested_destination: string;
}

export interface TriagePlan {
  source_directory: string;
  destination_base_directory: string;
  items: TriageItem[];
  category_counts: Record<string, number>;
  total_size_bytes: number;
  stale_installers_count: number;
  incomplete_downloads_count: number;
}

export interface TriageExecutionResult {
  moved_count: number;
  failed_items: [string, string][];
}

// ----------------------------------------------------------------------
// Phase 3 Intelligence Types
// ----------------------------------------------------------------------

export interface DuplicateExplanation {
  group_id: number;
  hash: string;
  size_bytes: number;
  total_members: number;
  match_level: string;
  confidence_percentage: number;
  keep_recommendation_reason: string;
  member_details: DuplicateMemberExplanation[];
}

export interface DuplicateMemberExplanation {
  file_id: number;
  file_name: string;
  path: string;
  modified_at_ms: number | null;
  copy_role: CopyRole;
  is_recommended_keep: boolean;
  has_duplicate_markers: boolean;
  path_depth: number;
  role_protection_note: string | null;
}

export interface ExtensionRectificationProposal {
  current_path: string;
  file_name: string;
  current_extension: string;
  detected_format: string;
  suggested_extension: string;
  suggested_new_path: string;
  confidence_percentage: number;
  reason: string;
}

export interface RectificationResultItem {
  original_path: string;
  rectified_path: string;
  success: boolean;
  error: string | null;
}

export interface RectificationExecutionResult {
  results: RectificationResultItem[];
}

export interface ImageFingerprint {
  path: string;
  file_name: string;
  file_size: number;
  dhash: number;
  width: number | null;
  height: number | null;
  format: string;
}

export interface NearDuplicateCluster {
  cluster_id: string;
  similarity_percentage: number;
  primary_keep_path: string;
  members: ImageFingerprint[];
  explanation: string;
}

// ----------------------------------------------------------------------
// Phase 5 Multi-Copy File Reconstruction Types
// ----------------------------------------------------------------------

export type BlockSource =
  | "SourceA"
  | "SourceB"
  | "IdenticalBoth"
  | "UnrecoverableNull"
  | "ConflictDifferentData";

export interface ReconstructionBlockMap {
  chunk_size: number;
  total_chunks: number;
  chunks_from_a: number;
  chunks_from_b: number;
  identical_chunks: number;
  unrecoverable_chunks: number;
  conflicting_chunks: number;
  block_sources: BlockSource[];
}

export interface ReconstructionPlan {
  candidate_a_path: string;
  candidate_b_path: string;
  output_path: string;
  file_size: number;
  block_map: ReconstructionBlockMap;
  potential_recovery_percentage: number;
  can_achieve_full_repair: boolean;
}

export interface ReconstructionResult {
  output_path: string;
  bytes_written: number;
  sha256: string;
  resulting_health_score: number;
  remaining_null_holes: number;
}

// ----------------------------------------------------------------------
// Phase 7 Document Lineage & Version Detection Types
// ----------------------------------------------------------------------

export type DocumentRevisionRole = "FinalVersion" | "ObsoleteDraft" | "Variant";

export interface DocumentFingerprint {
  path: string;
  file_name: string;
  file_size: number;
  modified_at_ms: number | null;
  word_count: number;
  simhash: number;
  format: string;
}

export interface DocumentLineageMember {
  fingerprint: DocumentFingerprint;
  role: DocumentRevisionRole;
  similarity_to_final: number;
}

export interface DocumentLineageCluster {
  cluster_id: string;
  similarity_percentage: number;
  primary_final_path: string;
  members: DocumentLineageMember[];
  explanation: string;
}




// ----------------------------------------------------------------------
// Phase 8 Carving & Harbor Patrol Types
// ----------------------------------------------------------------------

export interface CarvedFileProposal {
  source_path: string;
  original_name: string;
  detected_mime: string;
  detected_extension: string;
  extracted_date: string | null;
  extracted_camera_or_artist: string | null;
  extracted_title: string | null;
  proposed_name: string;
  proposed_path: string;
  confidence_score: number;
}

export interface CarveExecutionResult {
  succeeded: number;
  failed: number;
  errors: [string, string][];
}

export interface HarborPatrolSummary {
  backup_set_id: number;
  total_pool_objects: number;
  scanned_objects: number;
  healthy_objects: number;
  bit_rot_detected: number;
  self_healed_objects: number;
  unrecoverable_objects: number;
  healed_details: HealedObjectDetail[];
  unrecoverable_details: UnrecoverableDetail[];
}

export interface HealedObjectDetail {
  hash: string;
  source_path_used: string;
  pool_path: string;
  healed_at_ms: number;
}

export interface UnrecoverableDetail {
  hash: string;
  pool_path: string;
  affected_relative_paths: string[];
  affected_snapshot_ids: number[];
}
