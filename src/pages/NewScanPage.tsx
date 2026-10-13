import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { IconShield } from "../components/NavIcons";
import type { ScanSession } from "../types";
import { useEffect } from "react";

interface Props {
  initialRoots?: string[];
  onStarted: (sessionId: number) => void;
  onOpenResults: (sessionId: number) => void;
  activeScanId?: number | null;
}

export default function NewScanPage({ initialRoots, onStarted, onOpenResults, activeScanId }: Props) {
  const [roots, setRoots] = useState<string[]>(initialRoots ?? []);
  const [recent, setRecent] = useState<ScanSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [backupAdvisory, setBackupAdvisory] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (initialRoots && initialRoots.length > 0) {
      setRoots(initialRoots);
    }
  }, [initialRoots]);

  useEffect(() => {
    api.recentSessions().then(setRecent).catch(() => setRecent([]));
  }, []);
  useEffect(() => {
    let active = true;
    async function checkSafety() {
      for (const r of roots) {
        try {
          const setName = await api.checkBackupDestinationSafety(r);
          if (setName && active) {
            setBackupAdvisory(
              `The folder "${r}" contains FileHarbor backup data (${setName}). Backup files will automatically be recognized as protected copies and excluded from quarantine or cleanup recommendations.`
            );
            return;
          }
        } catch {
          // ignore
        }
      }
      if (active) {
        setBackupAdvisory(null);
      }
    }
    checkSafety();
    return () => {
      active = false;
    };
  }, [roots]);


  const addFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string" && !roots.includes(selected)) {
      setRoots([...roots, selected]);
    }
  };

  const startScan = async () => {
    if (roots.length === 0) {
      setError("Select at least one folder to scan.");
      return;
    }
    setStarting(true);
    setError(null);
    try {
      const { session_id } = await api.startScan(roots);
      onStarted(session_id);
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };

  return (
    <section>
      <h2>New Scan</h2>
      <p className="page-intro">
        Choose one or more folders to analyze. The scanner reads file metadata,
        detects empty files and finds exact duplicate files by content
        (SHA-256). Nothing is modified during a scan.
      </p>

      <div className="card">
        <h3>Folders to scan</h3>
        {roots.length === 0 ? (
          <p className="empty-inline">No folders selected yet.</p>
        ) : (
          <ul className="path-list">
            {roots.map((r) => (
              <li key={r}>
                <span className="mono">{r}</span>
                <button
                  className="btn btn-ghost btn-small"
                  aria-label={`Remove ${r}`}
                  onClick={() => setRoots(roots.filter((x) => x !== r))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
        {backupAdvisory && (
          <div className="backup-status-warning" role="status" style={{ margin: "12px 0" }}>
            <h4 style={{ display: "flex", alignItems: "center", gap: "6px", margin: "0 0 4px 0" }}>
              <IconShield size={16} />
              <span>Backup Location Detected</span>
            </h4>
            <p style={{ margin: 0, fontSize: "13px", color: "var(--text-dim)" }}>
              {backupAdvisory}
            </p>
          </div>
        )}
        <div className="row">
          <button className="btn btn-secondary" onClick={addFolder}>
            Add folder…
          </button>
          <button
            className="btn btn-primary"
            onClick={startScan}
            disabled={starting || roots.length === 0}
          >
            Start scan
          </button>
        </div>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </div>

      <div className="card">
        <h3>Previous scans</h3>
        {recent.length === 0 ? (
          <p className="empty-inline">No scans yet. Your first scan will appear here.</p>
        ) : (
          <table className="previous-scans-table">
            <thead>
              <tr>
                <th scope="col" style={{ width: "70px" }}>Scan</th>
                <th scope="col" style={{ width: "210px" }}>Started</th>
                <th scope="col" style={{ width: "130px" }}>Status</th>
                <th scope="col">Roots</th>
                <th scope="col" style={{ width: "140px", textAlign: "right" }}></th>
              </tr>
            </thead>
            <tbody>
              {recent.map((s) => {
                const isRunning = s.status === "running";
                const isStale = isRunning && s.id !== activeScanId;
                const statusLabel = isStale ? "interrupted" : s.status;
                return (
                  <tr key={s.id}>
                    <td className="scan-id-cell">#{s.id}</td>
                    <td className="scan-date-cell">
                      {new Date(s.started_at_ms).toLocaleString()}
                    </td>
                    <td>
                      <span className={`badge status-${statusLabel}`}>{statusLabel}</span>
                    </td>
                    <td className="scan-roots-cell">
                      <span className="mono truncated" title={s.roots.join(", ")}>
                        {s.roots.join(", ")}
                      </span>
                    </td>
                    <td style={{ textAlign: "right" }}>
                      <button className="btn btn-ghost btn-small" onClick={() => onOpenResults(s.id)}>
                        View results
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
