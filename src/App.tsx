import { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, PHASE_LABELS } from "./api";
import type { ScanFinished, ScanProgress } from "./types";
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
const DownloadsTriagePage = lazy(() =>
  import("./components/DownloadsTriagePage").then((m) => ({ default: m.DownloadsTriagePage }))
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

export default function App() {
  const [page, setPage] = useState<Page>("scan");
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [activeBackupSetId, setActiveBackupSetId] = useState<number | null>(null);
  const [activeBackupRunId, setActiveBackupRunId] = useState<number | null>(null);
  const [activeSnapshotId, setActiveSnapshotId] = useState<number | null>(null);
  const [initialScanRoots, setInitialScanRoots] = useState<string[] | undefined>(undefined);
  const [scan, setScan] = useState<ScanState | null>(null);
  const [finishedNotice, setFinishedNotice] = useState<ScanFinished | null>(null);
  const [toast, setToast] = useState<{ text: string; kind: "info" | "error" } | null>(null);

  const navigateTo = (
    targetPage: Page,
    opts?: { setId?: number; runId?: number; snapshotId?: number }
  ) => {
    if (opts?.setId !== undefined) setActiveBackupSetId(opts.setId);
    if (opts?.runId !== undefined) setActiveBackupRunId(opts.runId);
    if (opts?.snapshotId !== undefined) setActiveSnapshotId(opts.snapshotId);
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
        { id: "triage", label: "Downloads Triage" },
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
            />
          )}
          {page === "overview" && activeSessionId !== null && (
            <OverviewPage sessionId={activeSessionId} onNavigate={(p) => setPage(p)} />
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
          {page === "triage" && <DownloadsTriagePage />}
          {page === "health-recovery" && <HealthRecoveryPage />}
        </Suspense>
      </main>

      {scan && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true" aria-label="Scan in progress">
          <div className="modal scan-overlay">
            <h2>Scanning…</h2>
            <p className="phase">{phaseLabel}</p>
            {scan.progress ? (
              <>
                <dl className="scan-stats">
                  <div><dt>Discovered</dt><dd>{scan.progress.files_discovered.toLocaleString()}</dd></div>
                  <div><dt>Processed</dt><dd>{scan.progress.files_processed.toLocaleString()}</dd></div>
                  {scan.progress.candidates_total > 0 && (
                    <div>
                      <dt>Hashed candidates</dt>
                      <dd>
                        {scan.progress.candidates_hashed.toLocaleString()} /{" "}
                        {scan.progress.candidates_total.toLocaleString()}
                      </dd>
                    </div>
                  )}
                  {scan.progress.errors > 0 && (
                    <div><dt>Errors so far</dt><dd>{scan.progress.errors}</dd></div>
                  )}
                </dl>
                <p className="current-path mono" aria-live="polite">
                  {scan.progress.current_path || "\u00A0"}
                </p>
              </>
            ) : (
              <p>Starting scan…</p>
            )}
            {scan.progress?.phase === "done" ? (
              <>
                <p className="hint">Scan complete. Results are ready.</p>
                <button
                  className="btn btn-primary"
                  onClick={() => {
                    setActiveSessionId(scan.sessionId);
                    setPage("overview");
                    setScan(null);
                  }}
                >
                  View results
                </button>
              </>
            ) : (
              <>
                <button
                  className="btn btn-secondary"
                  onClick={() => api.cancelScan(scan.sessionId)}
                >
                  Cancel scan
                </button>
                <p className="hint">Cancellation is safe: partial results stay available.</p>
              </>
            )}
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
