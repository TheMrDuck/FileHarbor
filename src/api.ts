import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BatchReport,
  DuplicateGroup,
  FileOperation,
  FileRecord,
  OperationItem,
  QuarantineItem,
  RestorePreview,
  ScanSession,
  ScanSummary,
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
