import { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import type { BackupRun, BackupProgress, BackupRunStatus } from "../types";
import { formatBytes, formatDate } from "../api";

interface Props {
  runId?: number;
  onNavigate?: (page: string, opts?: { setId?: number }) => void;
}

interface BackupRunSummaryEvent {
  run_id: number;
  snapshot_id: number;
  status: BackupRunStatus;
  files_total: number;
  files_new: number;
  files_changed: number;
  files_unchanged: number;
  files_deleted: number;
  files_failed: number;
  files_verified: number;
  bytes_transferred: number;
  bytes_verified: number;
  duration_ms: number;
}

interface BackupFinishedEvent {
  run_id: number;
  status: string;
  summary?: BackupRunSummaryEvent;
  error?: string;
}

export default function BackupRunPage({ runId: propRunId, onNavigate }: Props) {
  const { id } = useParams<{ id: string }>();
  const runId = propRunId ?? parseInt(id || "0", 10);
  const [run, setRun] = useState<BackupRun | null>(null);
  const [progress, setProgress] = useState<BackupProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadRun();

    // Listen for backup progress events
    const unlistenProgress = listen<BackupProgress>("backup-progress", (event) => {
      if (event.payload.run_id === runId) {
        setProgress(event.payload);
      }
    });

    // Listen for backup finished events
    const unlistenFinished = listen<BackupFinishedEvent>("backup-finished", (event) => {
      if (event.payload.run_id === runId) {
        if (event.payload.summary) {
          const summary = event.payload.summary;
          setRun({
            id: summary.run_id,
            backup_set_id: 0, // Will be fetched from DB if needed
            snapshot_id: summary.snapshot_id,
            status: summary.status,
            started_at_ms: 0,
            finished_at_ms: Date.now(),
            files_total: summary.files_total,
            files_new: summary.files_new,
            files_changed: summary.files_changed,
            files_unchanged: summary.files_unchanged,
            files_deleted: summary.files_deleted,
            files_failed: summary.files_failed,
            bytes_transferred: summary.bytes_transferred,
            bytes_verified: summary.bytes_verified,
            error_summary: null,
          });
          setProgress(prev => prev ? {
            ...prev,
            status: summary.status,
            files_processed: summary.files_total,
            files_total: summary.files_total,
            bytes_transferred: summary.bytes_transferred,
            bytes_total: summary.files_total,
            bytes_verified: summary.bytes_verified,
            files_verified: summary.files_verified,
            files_failed: summary.files_failed,
            current_file: null,
          } : null);
        } else if (event.payload.error) {
          setError(event.payload.error);
        }
        setLoading(false);
      }
    });

    loadRun();
    return () => {
      unlistenProgress.then(f => f());
      unlistenFinished.then(f => f());
    };
  }, [runId]);

  const loadRun = async () => {
    try {
      if (runId) {
        const activeProgress = await api.getBackupProgress(runId);
        if (activeProgress) {
          setProgress(activeProgress);
        }
      }
      setLoading(false);
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  };

  const handleCancel = async () => {
    if (runId) {
      try {
        await api.cancelBackup(runId);
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const progressPercent = progress ? (progress.files_total > 0 ? (progress.files_processed / progress.files_total) * 100 : 0) : 0;

  if (loading) {
    return <section><p>Loading…</p></section>;
  }

  if (error) {
    return (
      <section>
        <p className="form-error" role="alert">Error: {error}</p>
        <button
          className="btn btn-secondary"
          onClick={() => onNavigate && onNavigate("backups")}
        >
          Back to Backups
        </button>
      </section>
    );
  }

  const currentStatus = progress?.status || run?.status || "running";

  return (
    <section className="backup-run">
      <header className="page-header">
        <h2>Backup in Progress</h2>
        <button
          className="btn btn-ghost"
          onClick={() => onNavigate && onNavigate("backups")}
        >
          Back
        </button>
      </header>

      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="card progress-card">
        <div className="progress-header">
          <h3>Backup Run #{runId}</h3>
          <span className={`badge status-${currentStatus}`}>
            {currentStatus}
          </span>
        </div>

        <div className="progress-bar-container">
          <div className="progress-bar" style={{ width: `${progressPercent}%` }} />
        </div>
        <p className="progress-text">
          {progress ? `${progress.files_processed.toLocaleString()} / ${progress.files_total.toLocaleString()} files` : "Starting…"}
        </p>

        <div className="progress-stats">
          <div className="stat">
            <span className="stat-value">{formatBytes(progress?.bytes_transferred || 0)}</span>
            <span className="stat-label">Transferred</span>
          </div>
          <div className="stat">
            <span className="stat-value">{formatBytes(progress?.bytes_verified || 0)}</span>
            <span className="stat-label">Verified</span>
          </div>
          <div className="stat">
            <span className="stat-value">{progress?.files_verified?.toLocaleString() || "0"}</span>
            <span className="stat-label">Files Verified</span>
          </div>
          <div className="stat">
            <span className="stat-value">{progress?.files_failed?.toLocaleString() || "0"}</span>
            <span className="stat-label">Failed</span>
          </div>
        </div>

        {progress?.current_file && (
          <p className="current-file mono truncated">{progress.current_file}</p>
        )}

        <div className="speed-info">
          {progress?.speed_bytes_per_sec && (
            <span>Speed: {(progress.speed_bytes_per_sec / (1024 * 1024)).toFixed(1)} MB/s</span>
          )}
          {progress?.eta_seconds && (
            <span>ETA: {Math.floor(progress.eta_seconds / 60)}m {progress.eta_seconds % 60}s</span>
          )}
        </div>

        <div className="run-actions">
          <button className="btn btn-danger" onClick={handleCancel} disabled={currentStatus !== "running" && currentStatus !== "verifying"}>
            Cancel Backup
          </button>
        </div>
      </div>

      <div className="card details-card">
        <h4>Details</h4>
        <table className="details-table">
          <tbody>
            <tr><th>Run ID</th><td className="mono">{runId}</td></tr>
            <tr><th>Status</th><td>{currentStatus}</td></tr>
            <tr><th>Started</th><td>{run?.started_at_ms ? formatDate(run.started_at_ms) : (progress ? "Active run" : "Just now")}</td></tr>
            <tr><th>Files Total</th><td className="mono">{run?.files_total?.toLocaleString() || progress?.files_total?.toLocaleString() || "0"}</td></tr>
            <tr><th>New Files</th><td className="mono">{run?.files_new?.toLocaleString() || "0"}</td></tr>
            <tr><th>Changed Files</th><td className="mono">{run?.files_changed?.toLocaleString() || "0"}</td></tr>
            <tr><th>Unchanged Files</th><td className="mono">{run?.files_unchanged?.toLocaleString() || "0"}</td></tr>
            <tr><th>Bytes to Transfer</th><td className="mono">{run?.bytes_transferred ? formatBytes(run.bytes_transferred) : "0 B"}</td></tr>
          </tbody>
        </table>
      </div>
    </section>
  );
}