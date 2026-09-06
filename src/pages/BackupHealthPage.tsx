import { useState, useEffect } from "react";
import { api } from "../api";
import type { BackupHealth, BackupSetHealth } from "../types";

export default function BackupHealthPage() {
  const [health, setHealth] = useState<BackupHealth | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadHealth();
  }, []);

  const loadHealth = async () => {
    setLoading(true);
    setError(null);
    try {
      const h = await api.getBackupHealth();
      setHealth(h);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  if (loading) return <section><p>Loading health data…</p></section>;

  if (error) return <section><p className="form-error">Error: {error}</p></section>;

  if (!health) return <section><p>No health data available</p></section>;

  return (
    <section className="backup-health">
      <header className="page-header">
        <h2>Backup Health</h2>
      </header>

      {/* Overall Protection Score */}
      <div className="card health-score-card">
        <h3>Overall Protection</h3>
        <div className="health-score">
          <div className="score-circle">
            <span className="score-value">{health.protection_percentage.toFixed(0)}%</span>
            <span className="score-label">Protected</span>
          </div>
          <div className="score-breakdown">
            <div className="breakdown-item protected">
              <span className="breakdown-count">{health.protected_files.toLocaleString()}</span>
              <span className="breakdown-label">Protected</span>
            </div>
            <div className="breakdown-item outdated">
              <span className="breakdown-count">{health.outdated_files.toLocaleString()}</span>
              <span className="breakdown-label">Outdated</span>
            </div>
            <div className="breakdown-item never">
              <span className="breakdown-count">{health.never_backed_up_files.toLocaleString()}</span>
              <span className="breakdown-label">Never Backed Up</span>
            </div>
            <div className="breakdown-item total">
              <span className="breakdown-count">{health.total_files.toLocaleString()}</span>
              <span className="breakdown-label">Total Files</span>
            </div>
          </div>
        </div>
      </div>

      {/* Per-Set Health */}
      <div className="card">
        <h3>Backup Sets</h3>
        {health.sets.length === 0 ? (
          <p className="empty-inline">No backup sets configured. Create your first backup to start protecting files.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th scope="col">Backup Set</th>
                <th scope="col">Status</th>
                <th scope="col">Protected</th>
                <th scope="col">Outdated</th>
                <th scope="col">Never Backed Up</th>
                <th scope="col">Protection</th>
              </tr>
            </thead>
            <tbody>
              {health.sets.map((s: BackupSetHealth) => (
                <tr key={s.backup_set_id}>
                  <td>
                    <strong>{s.name}</strong>
                  </td>
                  <td>
                    <span className={`badge status-${s.status}`}>{s.status}</span>
                  </td>
                  <td className="mono">{s.protected_files.toLocaleString()}</td>
                  <td className="mono">{s.outdated_files.toLocaleString()}</td>
                  <td className="mono">{s.never_backed_up_files.toLocaleString()}</td>
                  <td>
                    <div className="progress-mini">
                      <div className="progress-mini-bar" style={{ width: `${s.protection_percentage}%` }} />
                      <span>{s.protection_percentage.toFixed(0)}%</span>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Quick Actions */}
      <div className="card">
        <h3>Quick Actions</h3>
        <div className="quick-actions">
          <button className="btn btn-primary" onClick={() => window.location.hash = "/backup/create"}>
            Create New Backup
          </button>
          <button className="btn btn-secondary" onClick={() => window.location.hash = "/backups"}>
            View All Backups
          </button>
          <button className="btn btn-secondary" onClick={() => window.location.hash = "/settings"}>
            Configure Schedules
          </button>
        </div>
      </div>

      {/* Legend */}
      <div className="card legend">
        <h4>Legend</h4>
        <dl>
          <dt><span className="legend-color protected"></span>Protected</dt>
          <dd>File has a verified backup that matches the current version.</dd>
          <dt><span className="legend-color outdated"></span>Outdated</dt>
          <dd>File has a backup but the source has been modified since.</dd>
          <dt><span className="legend-color never"></span>Never Backed Up</dt>
          <dd>File has no backup at all.</dd>
        </dl>
      </div>
    </section>
  );
}