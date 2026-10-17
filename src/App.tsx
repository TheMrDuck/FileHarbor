import { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, PHASE_LABELS } from "./api";
import type { ScanFinished, ScanProgress, UpdateCheckResult } from "./types";
import ConfirmDialog from "./components/ConfirmDialog";
import { NavIcon } from "./components/NavIcons";
import NewScanPage from "./pages/NewScanPage";
import OverviewPage from "./pages/OverviewPage";
import DuplicatesPage from "./pages/DuplicatesPage";
import EmptyFilesPage from "./pages/EmptyFilesPage";
import ProblemsPage from "./pages/ProblemsPage";
import AllFilesPage from "./pages/AllFilesPage";
import QuarantinePage from "./pages/QuarantinePage";
import OperationsPage from "./pages/OperationsPage";
import SettingsPage from "./pages/SettingsPage";
import BackupsPage from "./pages/BackupsPage";
import BackupCreatePage from "./pages/BackupCreatePage";
import BackupRunPage from "./pages/BackupRunPage";
import BackupDetailsPage from "./pages/BackupDetailsPage";
import BackupRestorePage from "./pages/BackupRestorePage";
import BackupHistoryPage from "./pages/BackupHistoryPage";

const BackupHealthPage = lazy(() => import("./pages/BackupHealthPage"));
const TestDataPage = lazy(() => import("./pages/TestDataPage"));
const FolderTriagePage = lazy(() =>
  import("./components/DownloadsTriagePage").then((m) => ({ default: m.FolderTriagePage || m.DownloadsTriagePage }))
);
const HealthRecoveryPage = lazy(() =>
  import("./components/HealthRecoveryPage").then((m) => ({ default: m.HealthRecoveryPage }))
);

export type Page =
  | "scan"
  | "overview"
  | "duplicates"
  | "empty"
  | "problems"
  | "files"
  | "quarantine"
  | "operations"
  | "settings"
  | "backups"
  | "backup-create"
  | "backup-run"
  | "backup-details"
  | "backup-health"
  | "backup-restore"
  | "backup-history"
  | "test-data"
  | "triage"
  | "health-recovery";

interface ScanState {
  sessionId: number;
  progress: ScanProgress | null;
}

const SCAN_STEPS = [
  { key: "discovering", label: "Discover", number: 1 },
  { key: "finding_candidates", label: "Filter", number: 2 },
  { key: "hashing", label: "Hash", number: 3 },
  { key: "building_results", label: "Results", number: 4 },
];

export default function App() {
  const [page, setPage] = useState<Page>("scan");
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [activeBackupSetId, setActiveBackupSetId] = useState<number | null>(null);
  const [activeBackupRunId, setActiveBackupRunId] = useState<number | null>(null);
  const [activeSnapshotId, setActiveSnapshotId] = useState<number | null>(null);
  const [initialScanRoots, setInitialScanRoots] = useState<string[] | undefined>(undefined);
  const [selectedAuditFolder, setSelectedAuditFolder] = useState<string>("");
  const [updateAvailable, setUpdateAvailable] = useState<UpdateCheckResult | null>(null);
  const [scan, setScan] = useState<ScanState | null>(null);
  const [scanElapsedSeconds, setScanElapsedSeconds] = useState(0);
  const [finishedNotice, setFinishedNotice] = useState<ScanFinished | null>(null);
  const [toast, setToast] = useState<{ text: string; kind: "info" | "error" } | null>(null);

  useEffect(() => {
    api.checkForUpdates()
      .then((res) => {
        if (res.has_update) {
          setUpdateAvailable(res);
        }
      })
      .catch(() => {
        // Air-gapped or offline - ignore silently
      });
  }, []);

  useEffect(() => {
    if (!scan) {
      setScanElapsedSeconds(0);
      return;
    }
    const timer = setInterval(() => {
      setScanElapsedSeconds((prev) => prev + 1);
    }, 1000);
    return () => clearInterval(timer);
  }, [scan?.sessionId]);

  const navigateTo = (
    targetPage: Page,
    opts?: { setId?: number; runId?: number; snapshotId?: number; folder?: string }
  ) => {
    if (opts?.setId !== undefined) setActiveBackupSetId(opts.setId);
    if (opts?.runId !== undefined) setActiveBackupRunId(opts.runId);
    if (opts?.snapshotId !== undefined) setActiveSnapshotId(opts.snapshotId);
    if (opts?.folder !== undefined) setSelectedAuditFolder(opts.folder);
    setPage(targetPage);
  };

  const showToast = useCallback((text: string, kind: "info" | "error" = "info") => {
    setToast({ text, kind });
    window.setTimeout(() => setToast(null), 6000);
  }, []);

  useEffect(() => {
    const unlistenP = listen<ScanProgress>("scan-progress", (e) => {
      setScan((s) =>
        s && s.sessionId === e.payload.session_id ? { ...s, progress: e.payload } : s
      );
    });
    const unlistenF = listen<ScanFinished>("scan-finished", (e) => {
      setScan(null);
      setActiveSessionId(e.payload.session_id);
      if (e.payload.status === "completed") {
        setPage("overview");
      }
      setFinishedNotice(e.payload);
    });
    return () => {
      unlistenP.then((f) => f());
      unlistenF.then((f) => f());
    };
  }, []);

  const openResults = (sessionId: number) => {
    setActiveSessionId(sessionId);
    setPage("overview");
  };

  const navSections: {
    title: string;
    items: { id: Page; label: string; disabled?: boolean }[];
  }[] = [
    {
      title: "Analysis",
      items: [
        { id: "scan", label: "New Scan" },
        { id: "overview", label: "Overview", disabled: activeSessionId === null },
        { id: "duplicates", label: "Duplicates", disabled: activeSessionId === null },
        { id: "empty", label: "Empty Files", disabled: activeSessionId === null },
        { id: "problems", label: "Inaccessible", disabled: activeSessionId === null },
        { id: "files", label: "All Files", disabled: activeSessionId === null },
      ],
    },
    {
      title: "Action & Triage",
      items: [
        { id: "triage", label: "Folder Triage" },
        { id: "health-recovery", label: "Health & Carving" },
        { id: "quarantine", label: "Quarantine" },
      ],
    },
    {
      title: "Data Protection",
      items: [
        { id: "backups", label: "Verified Backups" },
        { id: "operations", label: "Audit History" },
      ],
    },
    {
      title: "System",
      items: [
        { id: "settings", label: "Settings" },
        { id: "test-data", label: "Test Sandbox" },
      ],
    },
  ];

  const currentPhaseKey = scan?.progress?.phase || "discovering";
  const currentStepIndex =
    currentPhaseKey === "discovering"
      ? 0
      : currentPhaseKey === "finding_candidates"
        ? 1
        : currentPhaseKey === "hashing"
          ? 2
          : currentPhaseKey === "building_results"
            ? 3
            : 4; // done

  let progressPercent = 0;
  let progressDetail = "";
  let currentSpeed = "";

  if (scan?.progress) {
    const { phase, files_discovered, candidates_total, candidates_hashed } = scan.progress;
    if (phase === "discovering") {
      progressPercent = files_discovered > 0 ? Math.min(25, Math.max(8, Math.floor(Math.log10(files_discovered + 1) * 5))) : 5;
      progressDetail = `${files_discovered.toLocaleString()} files indexed`;
      const speed = Math.round(files_discovered / Math.max(1, scanElapsedSeconds));
      currentSpeed = speed > 0 ? `~${speed.toLocaleString()} files/s` : "Indexing…";
    } else if (phase === "finding_candidates") {
      progressPercent = 30;
      progressDetail = "Grouping candidate sizes in database";
      currentSpeed = "Querying SQL";
    } else if (phase === "hashing") {
      const hashRatio = candidates_total > 0 ? candidates_hashed / candidates_total : 0;
      progressPercent = Math.min(90, Math.round(30 + hashRatio * 60));
      const pct = candidates_total > 0 ? Math.round(hashRatio * 100) : 0;
      progressDetail = `${candidates_hashed.toLocaleString()} / ${candidates_total.toLocaleString()} (${pct}%)`;
      const speed = Math.round(candidates_hashed / Math.max(1, scanElapsedSeconds));
      currentSpeed = speed > 0 ? `~${speed.toLocaleString()} hashes/s` : "Hashing…";
    } else if (phase === "building_results") {
      progressPercent = 95;
      progressDetail = "Building duplicate groups & metrics";
      currentSpeed = "Finalizing";
    } else if (phase === "done") {
      progressPercent = 100;
      progressDetail = "Scan complete";
      currentSpeed = "Done";
    }
  }

  const formatElapsed = (sec: number) => {
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  const phaseLabel = scan?.progress ? PHASE_LABELS[scan.progress.phase] ?? scan.progress.phase : "";

  return (
    <div className="app-shell">
      <nav className="sidebar" aria-label="Main navigation">
        <div className="brand">
          <svg className="brand-logo" width="22" height="22" viewBox="0 0 256 256" fill="none" aria-hidden="true">
            <rect width="256" height="256" rx="52" fill="#141820" stroke="#252C3B" strokeWidth="4" />
            <g transform="translate(56, 52)">
              <path d="M 0 0 L 28 0 L 28 152 L 0 152 Z" fill="#F8FAFC" />
              <path d="M 28 0 L 118 0 C 132 0, 144 12, 144 26 L 144 32 L 28 32 Z" fill="#F8FAFC" />
              <path d="M 28 62 L 96 62 C 104 62, 110 68, 110 76 L 110 90 L 28 90 Z" fill="#E2E8F0" />
              <path d="M 116 62 L 144 62 L 144 152 L 116 152 Z" fill="#E2E8F0" />
              <path d="M 46 118 C 62 108, 82 128, 98 120" stroke="#141820" strokeWidth="8" strokeLinecap="round" />
            </g>
          </svg>
          <h1>FileHarbor</h1>
        </div>
        <div className="nav-sections">
          {navSections.map((section) => (
            <div key={section.title} className="nav-section">
              <span className="nav-section-title">{section.title}</span>
              <ul>
                {section.items.map((item) => (
                  <li key={item.id}>
                    <button
                      className={page === item.id ? "nav-link active" : "nav-link"}
                      onClick={() => item.id !== page && setPage(item.id)}
                      disabled={item.disabled}
                      aria-current={page === item.id ? "page" : undefined}
                    >
                      <span className="nav-icon" aria-hidden="true">
                        <NavIcon page={item.id} />
                      </span>
                      <span>{item.label}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        {updateAvailable && (
          <div
            style={{
              margin: "12px 10px 4px",
              padding: "10px 12px",
              background: "linear-gradient(135deg, rgba(59, 130, 246, 0.15), rgba(99, 102, 241, 0.15))",
              border: "1px solid rgba(59, 130, 246, 0.3)",
              borderRadius: "8px",
              cursor: "pointer",
            }}
            onClick={() => setPage("settings")}
            title="Click to open Settings and download update"
          >
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
              <span style={{ color: "#60a5fa", fontWeight: 600, fontSize: "12px" }}>🚀 Update Ready</span>
              <span className="badge" style={{ fontSize: "10px", background: "#3b82f6", color: "#fff", padding: "1px 6px" }}>
                {updateAvailable.latest_version}
              </span>
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "4px" }}>
              Install new release via Settings →
            </div>
          </div>
        )}
        <p className="sidebar-note">
          File organization, exact duplicate detection, and verified backups.
          Nothing is ever deleted without your explicit confirmation.
        </p>
      </nav>

      <main className="content">
        <Suspense fallback={<div className="card" style={{ padding: "24px", color: "var(--text-dim)" }}>Loading view…</div>}>
          {page === "scan" && (
            <NewScanPage
              initialRoots={initialScanRoots}
              onStarted={(id) => setScan({ sessionId: id, progress: null })}
              onOpenResults={openResults}
              activeScanId={scan?.sessionId ?? null}
            />
          )}
          {page === "overview" && activeSessionId !== null && (
            <OverviewPage
              sessionId={activeSessionId}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "duplicates" && activeSessionId !== null && (
            <DuplicatesPage sessionId={activeSessionId} showToast={showToast} />
          )}
          {page === "empty" && activeSessionId !== null && (
            <EmptyFilesPage sessionId={activeSessionId} showToast={showToast} />
          )}
          {page === "problems" && activeSessionId !== null && (
            <ProblemsPage sessionId={activeSessionId} />
          )}
          {page === "files" && activeSessionId !== null && (
            <AllFilesPage sessionId={activeSessionId} />
          )}
          {page === "quarantine" && <QuarantinePage showToast={showToast} />}
          {page === "operations" && <OperationsPage />}
          {page === "settings" && <SettingsPage />}
          {page === "backups" && (
            <BackupsPage onNavigate={(p, opts) => navigateTo(p as Page, opts)} />
          )}
          {page === "backup-create" && (
            <BackupCreatePage
              onCancel={() => setPage("backups")}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "backup-run" && (
            <BackupRunPage
              runId={activeBackupRunId ?? undefined}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "backup-details" && (
            <BackupDetailsPage
              setId={activeBackupSetId ?? undefined}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "backup-health" && (
            <BackupHealthPage onNavigate={(p) => navigateTo(p as Page)} />
          )}
          {page === "backup-restore" && (
            <BackupRestorePage
              snapshotId={activeSnapshotId ?? undefined}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "backup-history" && (
            <BackupHistoryPage
              setId={activeBackupSetId ?? undefined}
              onNavigate={(p, opts) => navigateTo(p as Page, opts)}
            />
          )}
          {page === "test-data" && (
            <TestDataPage
              onNavigateToScan={(folderPath) => {
                setInitialScanRoots([folderPath]);
                setPage("scan");
              }}
            />
          )}
          {page === "triage" && <FolderTriagePage />}
          {page === "health-recovery" && (
            <HealthRecoveryPage initialFolderPath={selectedAuditFolder} />
          )}
        </Suspense>
      </main>

      {scan && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true" aria-label="Scan in progress">
          <div className="modal scan-overlay">
            <div className="scan-modal-header">
              <div className="scan-modal-title">
                <span className="scan-radar-pulse" aria-hidden="true" />
                <h2>{scan.progress?.phase === "done" ? "Scan Complete" : "Scanning Files…"}</h2>
              </div>
              <div className="scan-timer-badge">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="10" />
                  <polyline points="12 6 12 12 16 14" />
                </svg>
                <span>{formatElapsed(scanElapsedSeconds)}</span>
              </div>
            </div>

            {/* Stepper Pipeline */}
            <div className="scan-stepper">
              {SCAN_STEPS.map((step, idx) => {
                const isDone = currentStepIndex > idx;
                const isActive = currentStepIndex === idx;
                return (
                  <div key={step.key} className={`scan-step ${isDone ? "done" : isActive ? "active" : "pending"}`}>
                    <div className="scan-step-indicator">
                      {isDone ? (
                        <svg className="step-check" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                      ) : isActive ? (
                        <span className="step-spinner-dot" />
                      ) : (
                        <span>{step.number}</span>
                      )}
                    </div>
                    <span className="scan-step-label">{step.label}</span>
                    {idx < SCAN_STEPS.length - 1 && <div className={`scan-step-connector ${isDone ? "done" : ""}`} />}
                  </div>
                );
              })}
            </div>

            {/* Dual-Track Visual Progress Bar */}
            <div className="scan-progress-wrapper">
              <div className="scan-progress-track">
                <div
                  className={`scan-progress-fill ${currentPhaseKey === "discovering" ? "pulse-shimmer" : ""}`}
                  style={{ width: `${progressPercent}%` }}
                >
                  <div className="scan-progress-glow" />
                </div>
              </div>
              <div className="scan-progress-meta">
                <span className="scan-progress-phase-text">{phaseLabel}</span>
                <span className="scan-progress-percentage">{progressPercent}%</span>
              </div>
            </div>

            {scan.progress ? (
              <>
                {/* Bento Metrics Grid */}
                <div className="scan-stats-grid">
                  <div className="stat-card">
                    <span className="stat-label">Discovered</span>
                    <span className="stat-value">{scan.progress.files_discovered.toLocaleString()}</span>
                  </div>
                  <div className="stat-card">
                    <span className="stat-label">Candidate Hashing</span>
                    <span className="stat-value">
                      {scan.progress.candidates_total > 0
                        ? `${scan.progress.candidates_hashed.toLocaleString()} / ${scan.progress.candidates_total.toLocaleString()}`
                        : currentStepIndex > 1
                          ? "0 candidates"
                          : "Evaluating…"}
                    </span>
                  </div>
                  <div className="stat-card">
                    <span className="stat-label">Throughput</span>
                    <span className="stat-value">{currentSpeed}</span>
                  </div>
                  <div className="stat-card">
                    <span className="stat-label">Status</span>
                    <span className="stat-value highlight">{progressDetail}</span>
                  </div>
                </div>

                {scan.progress.errors > 0 && (
                  <div className="scan-error-callout">
                    <span className="error-icon">⚠️</span>
                    <span>{scan.progress.errors.toLocaleString()} inaccessible or skipped files</span>
                  </div>
                )}

                {/* Live File Stream Marquee */}
                <div className="scan-current-file">
                  <span className="file-stream-dot" aria-hidden="true" />
                  <span className="current-path mono" title={scan.progress.current_path}>
                    {scan.progress.current_path || "Scanning filesystem…"}
                  </span>
                </div>
              </>
            ) : (
              <div className="scan-starting-state">
                <div className="scan-spinner-ring" />
                <p>Initializing scan engine and analyzing target roots…</p>
              </div>
            )}

            {/* Actions */}
            <div className="scan-modal-actions">
              {scan.progress?.phase === "done" ? (
                <button
                  className="btn btn-primary btn-large"
                  onClick={() => {
                    setActiveSessionId(scan.sessionId);
                    setPage("overview");
                    setScan(null);
                  }}
                >
                  View Scan Results →
                </button>
              ) : (
                <div className="scan-actions-group">
                  <button
                    className="btn btn-secondary"
                    onClick={() => api.cancelScan(scan.sessionId)}
                  >
                    Cancel Scan
                  </button>
                  <p className="hint">Cancellation is safe: partial results stay available.</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {finishedNotice && (
        <ConfirmDialog
          title={
            finishedNotice.status === "cancelled"
              ? "Scan cancelled"
              : finishedNotice.status === "failed"
                ? "Scan failed"
                : "Scan complete"
          }
          body={
            finishedNotice.status === "failed"
              ? `The scan ended with an error: ${finishedNotice.error ?? "unknown error"}`
              : "Partial results are available for review."
          }
          confirmLabel="View results"
          cancelLabel="Close"
          danger={finishedNotice.status === "failed"}
          onConfirm={() => {
            openResults(finishedNotice.session_id);
            setFinishedNotice(null);
          }}
          onCancel={() => setFinishedNotice(null)}
        />
      )}

      {toast && (
        <div className={`toast ${toast.kind}`} role="status">
          {toast.text}
        </div>
      )}
    </div>
  );
}
