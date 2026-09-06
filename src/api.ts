import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BatchReport,
  BackupDestination,
  BackupHealth,
  BackupPlan,
  BackupProgress,
  BackupRun,
  BackupSet,
  BackupSnapshot,
  DestinationValidation,
  DuplicateGroup,
  FileOperation,
  FileRecord,
  FileBackupStatus,
  GetFileBackupStatusRequest,
  OperationItem,
  QuarantineItem,
  RestorePreview,
  RestoreResult,
  RestoreFromBackupRequest,
  ScanSession,
  ScanSummary,
  BackupSchedule,
  RetentionPolicy,
  VerificationMode,
} from "./types";

export const api = {
  startScan: (roots: string[]) =>
    invoke<{ session_id: number }>("start_scan", { request: { roots } }),
  cancelScan: (sessionId: number) => invoke<void>("cancel_scan", { sessionId }),
  recentSessions: () => invoke<ScanSession[]>("get_recent_sessions"),
  scanSummary: (sessionId: number) =>
    invoke<ScanSummary>("get_scan_summary", { sessionId }),

  listFiles: (request: {
    session_id: number;
    search?: string;
    extension?: string;
    problem_only?: boolean;
    empty_only?: boolean;
    sort?: string;
    ascending?: boolean;
    page?: number;
    page_size?: number;
  }) => invoke<{ items: FileRecord[]; total: number }>("list_files", { request }),

  problemFiles: (sessionId: number, page = 0) =>
    invoke<{ items: FileRecord[]; total: number }>("get_problem_files", {
      sessionId,
      page,
    }),

  duplicateGroups: (sessionId: number, page = 0) =>
    invoke<{ items: DuplicateGroup[]; total: number }>("get_duplicate_groups", {
      sessionId,
      page,
    }),

  rebuildAnalysis: (sessionId: number) =>
    invoke<void>("rebuild_analysis", { sessionId }),

  quarantineFiles: (fileIds: number[]) =>
    invoke<{ report: BatchReport }>("quarantine_files", { fileIds }),

  quarantineItems: () => invoke<QuarantineItem[]>("get_quarantine_items"),

  restorePreview: (itemIds: number[]) =>
    invoke<RestorePreview[]>("restore_preview", { itemIds }),

  restoreExecute: (itemIds: number[], resolution?: "keep_existing" | "new_name") =>
    invoke<{ report: BatchReport; kept_existing: string[] }>("restore_execute", {
      itemIds,
      resolution,
    }),

  deletePermanently: (itemIds: number[]) =>
    invoke<BatchReport>("delete_permanently", { itemIds }),

  operations: (page = 0) => invoke<FileOperation[]>("get_operations", { page }),
  operationItems: (operationId: number) =>
    invoke<OperationItem[]>("get_operation_items", { operationId }),

  exclusions: () => invoke<[number, string][]>("list_exclusions"),
  addExclusion: (path: string) => invoke<number>("add_exclusion", { path }),
  removeExclusion: (id: number) => invoke<void>("remove_exclusion", { id }),

  settings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) =>
    invoke<void>("save_settings", { settings }),

  // Backup sets
  createBackupSet: (request: {
    name: string;
    source_path: string;
    destination_path: string;
    schedule: BackupSchedule;
    retention_policy: RetentionPolicy;
    verification_mode: VerificationMode;
  }) => invoke<BackupSet>("create_backup_set", { request }),
  listBackupSets: () => invoke<BackupSet[]>("list_backup_sets"),
  getBackupSet: (setId: number) => invoke<BackupSet | null>("get_backup_set", { setId }),
  updateBackupSet: (setId: number, request: {
    name?: string;
    schedule?: BackupSchedule;
    retention_policy?: RetentionPolicy;
    verification_mode?: VerificationMode;
  }) => invoke<BackupSet>("update_backup_set", { setId, request }),
  deleteBackupSet: (setId: number, deleteData?: boolean) => invoke<void>("delete_backup_set", { setId, deleteData }),

  // Backup runs & snapshots
  getBackupRuns: (backupSetId: number, limit?: number) => invoke<BackupRun[]>("get_backup_runs", { backupSetId, limit }),
  getBackupSnapshots: (backupSetId: number) => invoke<BackupSnapshot[]>("get_backup_snapshots", { backupSetId }),

  // Destination validation
  validateBackupDestination: (sourcePath: string, destinationPath: string) =>
    invoke<DestinationValidation>("validate_backup_destination", { sourcePath, destinationPath }),

  // Planning & execution
  planBackup: (backupSetId: number) => invoke<BackupPlan>("plan_backup", { backupSetId }),
  startBackup: (backupSetId: number) => invoke<void>("start_backup", { backupSetId }),
  cancelBackup: (runId: number) => invoke<void>("cancel_backup", { runId }),
  getBackupProgress: (runId: number) => invoke<BackupProgress | null>("get_backup_progress", { runId }),
  verifyBackup: (runId: number) => invoke<{ runId: number; snapshotId: number; totalFiles: number; verifiedFiles: number; failedFiles: number; missingFiles: number; errors: { relativePath: string; error: string; expectedHash: string | null; actualHash: string | null }[] }>("verify_backup", { runId }),
  getBackupHealth: () => invoke<BackupHealth>("get_backup_health"),

  // Restore
  restoreFromBackup: (request: RestoreFromBackupRequest) => invoke<RestoreResult>("restore_from_backup", { request }),
  getRestorePreview: (snapshotId: number, destinationRoot: string) => invoke<RestorePreview>("get_restore_preview", { snapshotId, destinationRoot }),
  retryFailedBackupItems: (runId: number) => invoke<void>("retry_failed_backup_items", { runId }),

  // Destinations
  listBackupDestinations: () => invoke<BackupDestination[]>("list_backup_destinations"),

  // File backup status
  getFileBackupStatus: (request: GetFileBackupStatusRequest) =>
    invoke<FileBackupStatus[]>("get_file_backup_status", { request }),

  // Backup destination safety
  checkBackupDestinationSafety: (path: string) =>
    invoke<string | null>("check_backup_destination_safety", { path }),

  // Check if a file has a verified backup
  checkFileBackupStatus: (file_path: string) =>
    invoke<[string, string] | null>("check_file_backup_status", { file_path }),
};

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes)) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

export function formatDate(ms: number | null | undefined): string {
  if (!ms) return "-";
  return new Date(ms).toLocaleString();
}

export const PHASE_LABELS: Record<string, string> = {
  discovering: "1/4 · Discovering files",
  finding_candidates: "2/4 · Finding duplicate candidates",
  hashing: "3/4 · Hashing candidate files",
  building_results: "4/4 · Building results",
  done: "Finished",
};