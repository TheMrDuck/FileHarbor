import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { ScanSession } from "../types";
import { useEffect } from "react";

interface Props {
  onStarted: (sessionId: number) => void;
  onOpenResults: (sessionId: number) => void;
}

export default function NewScanPage({ onStarted, onOpenResults }: Props) {
  const [roots, setRoots] = useState<string[]>([]);
  const [recent, setRecent] = useState<ScanSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    api.recentSessions().then(setRecent).catch(() => setRecent([]));
  }, []);

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
          <p className="empty-inline">No scans yet — your first scan will appear here.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">Scan</th>
                <th scope="col">Started</th>
                <th scope="col">Status</th>
                <th scope="col">Roots</th>
                <th scope="col"></th>
              </tr>
            </thead>
            <tbody>
              {recent.map((s) => (
                <tr key={s.id}>
                  <td>#{s.id}</td>
                  <td>{new Date(s.started_at_ms).toLocaleString()}</td>
                  <td>
                    <span className={`badge status-${s.status}`}>{s.status}</span>
                  </td>
                  <td className="mono truncated">{s.roots.join(", ")}</td>
                  <td>
                    <button className="btn btn-ghost btn-small" onClick={() => onOpenResults(s.id)}>
                      View results
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
