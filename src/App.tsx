import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, PHASE_LABELS } from "./api";
import type { ScanFinished, ScanProgress } from "./types";
import ConfirmDialog from "./components/ConfirmDialog";
import NewScanPage from "./pages/NewScanPage";
import OverviewPage from "./pages/OverviewPage";
import DuplicatesPage from "./pages/DuplicatesPage";
import EmptyFilesPage from "./pages/EmptyFilesPage";
import ProblemsPage from "./pages/ProblemsPage";
import AllFilesPage from "./pages/AllFilesPage";
import QuarantinePage from "./pages/QuarantinePage";
import OperationsPage from "./pages/OperationsPage";
import SettingsPage from "./pages/SettingsPage";

export type Page =
  | "scan"
  | "overview"
  | "duplicates"
  | "empty"
  | "problems"
  | "files"
  | "quarantine"
  | "operations"
  | "settings";

interface ScanState {
  sessionId: number;
  progress: ScanProgress | null;
}

export default function App() {
  const [page, setPage] = useState<Page>("scan");
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [scan, setScan] = useState<ScanState | null>(null);
  const [finishedNotice, setFinishedNotice] = useState<ScanFinished | null>(null);
  const [toast, setToast] = useState<{ text: string; kind: "info" | "error" } | null>(null);

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

  const navItems: { id: Page; label: string; disabled?: boolean }[] = [
    { id: "scan", label: "New Scan" },
    { id: "overview", label: "Overview", disabled: activeSessionId === null },
    { id: "duplicates", label: "Duplicates", disabled: activeSessionId === null },
    { id: "empty", label: "Empty Files", disabled: activeSessionId === null },
    { id: "problems", label: "Inaccessible & Errors", disabled: activeSessionId === null },
    { id: "files", label: "All Files", disabled: activeSessionId === null },
    { id: "quarantine", label: "Quarantine" },
    { id: "operations", label: "Operation History" },
    { id: "settings", label: "Settings" },
  ];

  const phaseLabel = scan?.progress ? PHASE_LABELS[scan.progress.phase] ?? scan.progress.phase : "";

  return (
    <div className="app-shell">
      <nav className="sidebar" aria-label="Main navigation">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true" />
          <h1>FileHarbor</h1>
        </div>
        <ul>
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                className={page === item.id ? "nav-link active" : "nav-link"}
                onClick={() => item.id !== page && setPage(item.id)}
                disabled={item.disabled}
                aria-current={page === item.id ? "page" : undefined}
              >
                {item.label}
              </button>
            </li>
          ))}
        </ul>
        <p className="sidebar-note">
          Phase 1 finds facts only: exact duplicates and empty files.
          Nothing is ever deleted without your explicit confirmation.
        </p>
      </nav>

      <main className="content">
        {page === "scan" && (
          <NewScanPage onStarted={(id) => setScan({ sessionId: id, progress: null })} onOpenResults={openResults} />
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
            <button
              className="btn btn-secondary"
              onClick={() => api.cancelScan(scan.sessionId)}
            >
              Cancel scan
            </button>
            <p className="hint">Cancellation is safe — partial results stay available.</p>
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
