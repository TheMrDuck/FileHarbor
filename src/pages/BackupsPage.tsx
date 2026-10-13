import { useState, useEffect } from "react";
import { api } from "../api";
import type { BackupSet, BackupRun } from "../types";
import { formatBytes, formatDate } from "../api";

interface Props {
  onNavigate: (page: string, opts?: { setId?: number }) => void;
}

export default function BackupsPage({ onNavigate }: Props) {
  const [sets, setSets] = useState<BackupSet[]>([]);
  const [destinations, setDestinations] = useState<Array<{ id: number; path: string; label: string | null }>>([]);
  const [runs, setRuns] = useState<BackupRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [setsRes, destsRes, runsRes] = await Promise.all([
        api.listBackupSets(),
        api.listBackupDestinations(),
        api.getBackupRuns(0, 20), // Recent runs across all sets
      ]);
      setSets(setsRes);
      setDestinations(destsRes);
      setRuns(runsRes);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCreateBackup = () => {
    onNavigate("backup-create");
  };

  const formatStatus = (status: string) => {
    const badges: Record<string, string> = {
      completed: "badge status-completed",
      failed: "badge status-failed",
      partial: "badge status-partial",
      running: "badge status-running",
      verifying: "badge status-verifying",
      committing: "badge status-committing",
      cancelled: "badge status-cancelled",
      active: "badge status-active",
      paused: "badge status-paused",
      disabled: "badge status-disabled",
      verified: "badge status-verified",
      unverified: "badge status-unverified",
      corrupted: "badge status-corrupted",
    };
    return <span className={badges[status] || "badge"}>{status}</span>;
  };

  if (loading) {
    return <section><p>Loading backups…</p></section>;
  }

  return (
    <section>
      <header className="page-header">
        <h2>Backups</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={() => onNavigate("backup-health")}>
            Backup Health
          </button>
          <button className="btn btn-primary" onClick={handleCreateBackup}>
            Create Backup
          </button>
        </div>
      </header>

      {error && (
        <p className="form-error" role="alert">{error}</p>
      )}

      {/* Backup Sets */}
      <div className="card">
        <h3>Backup Sets</h3>
        {sets.length === 0 ? (
          <p className="empty-inline">No backup sets configured. Create your first backup to protect important files.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Source</th>
                <th scope="col">Destination</th>
                <th scope="col">Schedule</th>
                <th scope="col">Status</th>
                <th scope="col">Last Run</th>
                <th scope="col"></th>
              </tr>
            </thead>
            <tbody>
              {sets.map((s) => {
                const dest = destinations.find((d) => d.id === s.destination_id);
                const lastRun = runs.find((r) => r.backup_set_id === s.id);
                return (
                  <tr key={s.id}>
                    <td className="mono">{s.name}</td>
                    <td className="truncated">{s.source_path}</td>
                    <td className="truncated">{dest?.path || "Unknown"}</td>
                    <td><span className="badge">{s.schedule}</span></td>
                    <td>{formatStatus(s.schedule)}</td>
                    <td>{lastRun ? formatDate(lastRun.started_at_ms) : "Never"}</td>
                    <td>
                      <button className="btn btn-ghost btn-small" onClick={() => onNavigate("backup-details", { setId: s.id })}>
                        Details
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Recent Activity */}
      <div className="card">
        <h3>Recent Backup Activity</h3>
        {runs.length === 0 ? (
          <p className="empty-inline">No backup runs yet.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">Backup Set</th>
                <th scope="col">Started</th>
                <th scope="col">Status</th>
                <th scope="col">Files</th>
                <th scope="col">Transferred</th>
                <th scope="col">Verified</th>
              </tr>
            </thead>
            <tbody>
              {runs.slice(0, 10).map((r) => {
                const set = sets.find((s) => s.id === r.backup_set_id);
                return (
                  <tr key={r.id}>
                    <td>{set?.name || `Set #${r.backup_set_id}`}</td>
                    <td>{formatDate(r.started_at_ms)}</td>
                    <td>{formatStatus(r.status)}</td>
                    <td className="mono">{r.files_total.toLocaleString()}</td>
                    <td className="mono">{formatBytes(r.bytes_transferred)}</td>
                    <td className="mono">{formatBytes(r.bytes_verified)}</td>
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