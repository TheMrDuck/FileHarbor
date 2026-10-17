import { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { api } from "../api";
import type { BackupRun, BackupSnapshot, BackupSet } from "../types";
import { formatBytes, formatDate } from "../api";
import ConfirmDialog from "../components/ConfirmDialog";

interface Props {
  setId?: number;
  onNavigate?: (page: string, opts?: { setId?: number; runId?: number; snapshotId?: number }) => void;
}

export default function BackupHistoryPage({ setId: propSetId, onNavigate }: Props) {
  const { id } = useParams<{ id: string }>();
  const setId = propSetId ?? parseInt(id || "0", 10);
  const [set, setSet] = useState<BackupSet | null>(null);
  const [runs, setRuns] = useState<BackupRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"runs" | "snapshots">("runs");
  const [snapshots, setSnapshots] = useState<BackupSnapshot[]>([]);
  const [destination, setDestination] = useState<{ id: number; path: string; label: string | null; filesystem_id: string | null; created_at_ms: number; last_seen_at_ms: number | null } | null>(null);
  const [confirmDeleteConfig, setConfirmDeleteConfig] = useState(false);
  const [confirmDeleteAll, setConfirmDeleteAll] = useState(false);

  useEffect(() => {
    loadData();
  }, [setId]);

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [setRes, runsRes, snapshotsRes] = await Promise.all([
        api.getBackupSet(setId),
        api.getBackupRuns(setId, 100),
        api.getBackupSnapshots(setId),
      ]);
      if (setRes) {
        setSet(setRes);
        const dests = await api.listBackupDestinations();
        const dest = dests.find((d) => d.id === setRes.destination_id);
        if (dest) setDestination(dest);
      } else {
        setError("Backup set not found");
      }
      setRuns(runsRes);
      setSnapshots(snapshotsRes);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const [starting, setStarting] = useState(false);

  const handleStartBackup = async () => {
    if (!set) return;
    setStarting(true);
    setError(null);
    try {
      const runId = await api.startBackup(set.id);
      if (onNavigate) {
        onNavigate("backup-run", { runId, setId: set.id });
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };

  const executeDeleteConfig = async () => {
    setConfirmDeleteConfig(false);
    try {
      await api.deleteBackupSet(setId, false);
      if (onNavigate) onNavigate("backups");
    } catch (e) {
      setError(String(e));
    }
  };

  const executeDeleteAll = async () => {
    setConfirmDeleteAll(false);
    try {
      await api.deleteBackupSet(setId, true);
      if (onNavigate) onNavigate("backups");
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) return <section><p>Loading…</p></section>;

  if (error) return <section><p className="form-error">Error: {error}</p></section>;

  if (!set) return <section><p>Backup set not found</p></section>;

  return (
    <section className="backup-history">
      <header className="page-header">
        <h2>{set.name} - History</h2>
        <div className="header-actions">
          <button
            className="btn btn-primary"
            onClick={handleStartBackup}
            disabled={starting}
          >
            {starting ? "Starting…" : "Run Backup"}
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => onNavigate && onNavigate("backup-details", { setId: set.id })}
          >
            Back
          </button>
        </div>
      </header>

      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="grid grid-2">
        <div className="card">
          <h3>Configuration</h3>
          <dl className="details-list">
            <dt>Name</dt><dd className="mono">{set.name}</dd>
            <dt>Source</dt><dd className="truncated mono">{set.source_path}</dd>
            <dt>Destination</dt><dd className="truncated mono">{destination?.path || "Unknown"}</dd>
            <dt>Schedule</dt><dd><span className="badge">{set.schedule}</span></dd>
            <dt>Retention</dt><dd>{set.retention_policy}</dd>
            <dt>Verification</dt><dd>{set.verification_mode}</dd>
            <dt>Created</dt><dd>{formatDate(set.created_at_ms)}</dd>
            <dt>Updated</dt><dd>{formatDate(set.updated_at_ms)}</dd>
          </dl>
        </div>

        <div className="card">
          <h3>Destination Details</h3>
          {destination && (
            <dl className="details-list">
              <dt>Path</dt><dd className="truncated mono">{destination.path}</dd>
              <dt>Label</dt><dd>{destination.label || "None"}</dd>
              <dt>Filesystem ID</dt><dd className="mono">{destination.filesystem_id || "None"}</dd>
              <dt>Created</dt><dd>{formatDate(destination.created_at_ms)}</dd>
              <dt>Last Seen</dt><dd>{destination.last_seen_at_ms ? formatDate(destination.last_seen_at_ms) : "Never"}</dd>
            </dl>
          )}
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <h3>{activeTab === "runs" ? "Backup Runs" : "Snapshots"}</h3>
          <div className="tabs">
            <button className={`tab ${activeTab === "runs" ? "active" : ""}`} onClick={() => setActiveTab("runs")}>Runs</button>
            <button className={`tab ${activeTab === "snapshots" ? "active" : ""}`} onClick={() => setActiveTab("snapshots")}>Snapshots</button>
          </div>
        </div>

        {activeTab === "runs" && (
          <div className="table-container">
            {runs.length === 0 ? (
              <p className="empty-inline">No backup runs yet.</p>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th scope="col">Run ID</th>
                    <th scope="col">Started</th>
                    <th scope="col">Status</th>
                    <th scope="col">Files</th>
                    <th scope="col">Transferred</th>
                    <th scope="col">Verified</th>
                    <th scope="col">Failed</th>
                  </tr>
                </thead>
                <tbody>
                  {runs.map((r) => (
                    <tr key={r.id}>
                      <td className="mono">#{r.id}</td>
                      <td>{formatDate(r.started_at_ms)}</td>
                      <td><span className={`badge status-${r.status}`}>{r.status}</span></td>
                      <td className="mono">{r.files_total.toLocaleString()}</td>
                      <td className="mono">{formatBytes(r.bytes_transferred)}</td>
                      <td className="mono">{formatBytes(r.bytes_verified)}</td>
                      <td className="mono">{r.files_failed.toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}

        {activeTab === "snapshots" && (
          <div className="table-container">
            {snapshots.length === 0 ? (
              <p className="empty-inline">No snapshots yet. Run a backup to create the first snapshot.</p>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th scope="col">Snapshot</th>
                    <th scope="col">Created</th>
                    <th scope="col">Status</th>
                    <th scope="col">Files</th>
                    <th scope="col">Size</th>
                  </tr>
                </thead>
                <tbody>
                  {snapshots.map((s) => (
                    <tr key={s.id}>
                      <td className="mono">{s.label}</td>
                      <td>{formatDate(s.created_at_ms)}</td>
                      <td><span className={`badge status-${s.status}`}>{s.status}</span></td>
                      <td className="mono">{s.files_count.toLocaleString()}</td>
                      <td className="mono">{formatBytes(s.total_size_bytes)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}
      </div>

      <div className="card danger-zone">
        <h3>Danger Zone</h3>
        <p>These actions are irreversible.</p>
        <div className="danger-actions">
          <button className="btn btn-danger" onClick={() => setConfirmDeleteConfig(true)}>
            Delete Configuration Only
          </button>
          <button className="btn btn-danger btn-destructive" onClick={() => setConfirmDeleteAll(true)}>
            Delete Configuration & Data
          </button>
        </div>
      </div>

      {confirmDeleteConfig && (
        <ConfirmDialog
          title="Delete Backup Configuration?"
          body="Are you sure you want to delete this backup set configuration? The backup files already stored in the destination folder will remain intact."
          confirmLabel="Delete Configuration"
          danger
          onCancel={() => setConfirmDeleteConfig(false)}
          onConfirm={executeDeleteConfig}
        />
      )}

      {confirmDeleteAll && (
        <ConfirmDialog
          title="Delete Configuration & Permanent Backup Data?"
          body="WARNING: This will permanently erase the backup configuration and purge all snapshot files in the destination storage pool. This cannot be undone."
          confirmLabel="Permanently Delete Everything"
          danger
          onCancel={() => setConfirmDeleteAll(false)}
          onConfirm={executeDeleteAll}
        />
      )}
    </section>
  );
}