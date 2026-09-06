import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { api } from "../api";
import type { BackupSet, BackupRun, BackupSnapshot } from "../types";
import { formatBytes, formatDate } from "../api";

export default function BackupDetailsPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const setId = parseInt(id || "0", 10);
  const [set, setSet] = useState<BackupSet | null>(null);
  const [destination, setDestination] = useState<{ id: number; path: string; label: string | null; filesystem_id: string | null; created_at_ms: number; last_seen_at_ms: number | null } | null>(null);
  const [runs, setRuns] = useState<BackupRun[]>([]);
  const [snapshots, setSnapshots] = useState<BackupSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"runs" | "snapshots">("runs");

  useEffect(() => {
    loadData();
  }, [setId]);

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [setRes, runsRes, snapshotsRes] = await Promise.all([
        api.getBackupSet(setId),
        api.getBackupRuns(setId, 50),
        api.getBackupSnapshots(setId),
      ]);
      if (setRes) {
        setSet(setRes);
        // Get destination
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

  const handleDelete = async () => {
    if (!confirm("Delete this backup set? This will remove the configuration but keep the backup data.")) return;
    try {
      await api.deleteBackupSet(setId, false);
      navigate("/backups");
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDeleteWithData = async () => {
    if (!confirm("Delete this backup set AND all backup data? This cannot be undone!")) return;
    if (!confirm("Are you absolutely sure? All backup files will be permanently deleted.")) return;
    try {
      await api.deleteBackupSet(setId, true);
      navigate("/backups");
    } catch (e) {
      setError(String(e));
    }
  };

  const handleVerifyBackup = async () => {
    if (runs.length === 0) {
      setError("No backup runs available to verify.");
      return;
    }
    try {
      setError(null);
      const result = await api.verifyBackup(runs[0].id);
      if (result.failedFiles > 0 || result.missingFiles > 0) {
        setError(`Verification completed with issues: ${result.failedFiles} failed, ${result.missingFiles} missing. ${result.verifiedFiles} files verified.`);
      } else {
        alert(`Backup verified successfully! ${result.verifiedFiles} files verified.`);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) return <section><p>Loading…</p></section>;

  if (error) return <section><p className="form-error">Error: {error}</p></section>;

  if (!set) return <section><p>Backup set not found</p></section>;

  return (
    <section className="backup-details">
      <header className="page-header">
        <h2>{set.name}</h2>
        <div className="header-actions">
          <button className="btn btn-primary" onClick={() => navigate(`/backup/${set.id}/run`)}>
            Start Backup
          </button>
          <button className="btn btn-ghost" onClick={() => navigate("/backups")}>Back</button>
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
              <dt>Label</dt><dd>{destination.label || "—"}</dd>
              <dt>Filesystem ID</dt><dd className="mono">{destination.filesystem_id || "—"}</dd>
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
                    <th scope="col"></th>
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
                      <td>
                        <button className="btn btn-secondary btn-small" onClick={() => navigate(`/backup/${s.id}/restore`)}>
                          Restore
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}
      </div>

      <div className="card danger-zone">
        <h3>Actions</h3>
        <p>These actions may take a long time to complete.</p>
        <div className="danger-actions">
          <button className="btn btn-secondary" onClick={handleVerifyBackup}>
            Verify Backup
          </button>
        </div>
        <h3>Danger Zone</h3>
        <p>These actions are irreversible.</p>
        <div className="danger-actions">
          <button className="btn btn-danger" onClick={handleDelete}>
            Delete Configuration Only
          </button>
          <button className="btn btn-danger btn-destructive" onClick={handleDeleteWithData}>
            Delete Configuration & Data
          </button>
        </div>
      </div>
    </section>
  );
}