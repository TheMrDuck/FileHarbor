import { useState, useEffect } from "react";
import { api } from "../api";
import { IconShield, IconAlert, IconCheck } from "../components/NavIcons";
import type { BackupHealth, BackupSetHealth, HarborPatrolSummary } from "../types";

interface Props {
  onNavigate?: (page: string) => void;
}

export default function BackupHealthPage({ onNavigate }: Props) {
  const [health, setHealth] = useState<BackupHealth | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Harbor Patrol State
  const [selectedPatrolSetId, setSelectedPatrolSetId] = useState<number | null>(null);
  const [patrolRunning, setPatrolRunning] = useState<boolean>(false);
  const [patrolSummary, setPatrolSummary] = useState<HarborPatrolSummary | null>(null);
  const [patrolError, setPatrolError] = useState<string | null>(null);

  useEffect(() => {
    loadHealth();
  }, []);

  const loadHealth = async () => {
    setLoading(true);
    setError(null);
    try {
      const h = await api.getBackupHealth();
      setHealth(h);
      if (h.sets.length > 0 && selectedPatrolSetId === null) {
        setSelectedPatrolSetId(h.sets[0].backup_set_id);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleRunPatrol = async () => {
    if (!selectedPatrolSetId) return;
    setPatrolRunning(true);
    setPatrolError(null);
    try {
      const summary = await api.runHarborPatrol(selectedPatrolSetId, true);
      setPatrolSummary(summary);
      // Reload health to reflect any healed or updated snapshot states
      loadHealth();
    } catch (e) {
      setPatrolError(String(e));
    } finally {
      setPatrolRunning(false);
    }
  };

  if (loading && !health) return <section><p>Loading health data…</p></section>;

  if (error) return <section><p className="form-error">Error: {error}</p></section>;

  if (!health) return <section><p>No health data available</p></section>;

  return (
    <section className="backup-health" style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      <header className="page-header">
        <h2>Backup Health & Integrity</h2>
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

      {/* HARBOR PATROL: BIT-ROT SCRUBBING & SELF-HEALING */}
      <div className="card" style={{ border: "1px solid rgba(56, 189, 248, 0.3)", background: "rgba(56, 189, 248, 0.02)" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "12px" }}>
          <div>
            <h3 style={{ margin: 0, display: "flex", alignItems: "center", gap: "8px" }}>
              <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                <IconShield size={18} />
                <span>Harbor Patrol: Bit-Rot Scrubber & Self-Healing</span>
              </span>
              <span className="badge" style={{ background: "rgba(16, 185, 129, 0.15)", color: "var(--success)", fontSize: "11px" }}>
                Auto-Repair Enabled
              </span>
            </h3>
            <p style={{ margin: "4px 0 0 0", fontSize: "13px", color: "var(--text-dim)" }}>
              Performs cryptographic block audits over immutable content-addressed storage (.fileharbor/pool).
              If silent disk bit-rot is detected, Harbor Patrol autonomously repairs corrupted objects using active source replicas.
            </p>
          </div>

          {health.sets.length > 0 && (
            <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
              <select
                value={selectedPatrolSetId || undefined}
                onChange={(e) => setSelectedPatrolSetId(Number(e.target.value))}
                aria-label="Backup set to inspect with Harbor Patrol"
                style={{
                  padding: "6px 12px",
                  background: "var(--bg)",
                  color: "var(--text)",
                  border: "1px solid var(--border)",
                  borderRadius: "var(--radius)",
                }}
              >
                {health.sets.map((s) => (
                  <option key={s.backup_set_id} value={s.backup_set_id}>
                    {s.name}
                  </option>
                ))}
              </select>
              <button
                className="btn btn-primary"
                onClick={handleRunPatrol}
                disabled={patrolRunning || !selectedPatrolSetId}
              >
                {patrolRunning ? "Scrubbing Pool..." : "Run Harbor Patrol"}
              </button>
            </div>
          )}
        </div>

        {patrolError && (
          <div className="notice-banner danger" style={{ padding: "10px 14px", marginTop: "10px", display: "flex", alignItems: "center", gap: "6px" }}>
            <IconAlert size={14} />
            <span>Patrol Error: {patrolError}</span>
          </div>
        )}

        {patrolSummary && (
          <div style={{ marginTop: "16px", display: "flex", flexDirection: "column", gap: "12px" }}>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: "12px", background: "rgba(0,0,0,0.15)", padding: "12px", borderRadius: "var(--radius)" }}>
              <div>
                <div style={{ fontSize: "11px", color: "var(--text-dim)" }}>Scanned Pool Objects</div>
                <div style={{ fontWeight: 700, fontSize: "16px" }}>{patrolSummary.scanned_objects}</div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: "var(--success)" }}>Cryptographically Clean</div>
                <div style={{ fontWeight: 700, fontSize: "16px", color: "var(--success)" }}>{patrolSummary.healthy_objects}</div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: patrolSummary.bit_rot_detected > 0 ? "var(--warning)" : "var(--text-dim)" }}>
                  Bit-Rot Faults Detected
                </div>
                <div style={{ fontWeight: 700, fontSize: "16px", color: patrolSummary.bit_rot_detected > 0 ? "var(--warning)" : "inherit" }}>
                  {patrolSummary.bit_rot_detected}
                </div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: "var(--success)" }}>Autonomously Healed</div>
                <div style={{ fontWeight: 700, fontSize: "16px", color: "var(--success)" }}>
                  {patrolSummary.self_healed_objects}
                </div>
              </div>
              <div>
                <div style={{ fontSize: "11px", color: patrolSummary.unrecoverable_objects > 0 ? "var(--danger)" : "var(--text-dim)" }}>
                  Unrecoverable Objects
                </div>
                <div style={{ fontWeight: 700, fontSize: "16px", color: patrolSummary.unrecoverable_objects > 0 ? "var(--danger)" : "inherit" }}>
                  {patrolSummary.unrecoverable_objects}
                </div>
              </div>
            </div>

            {patrolSummary.healed_details.length > 0 && (
              <div style={{ background: "rgba(16, 185, 129, 0.06)", border: "1px solid rgba(16, 185, 129, 0.3)", borderRadius: "var(--radius)", padding: "12px" }}>
                <h4 style={{ margin: "0 0 6px 0", color: "var(--success)", display: "flex", alignItems: "center", gap: "6px" }}>
                  <IconCheck size={16} />
                  <span>Autonomously Repaired Objects ({patrolSummary.healed_details.length})</span>
                </h4>
                <div style={{ fontSize: "12px", display: "flex", flexDirection: "column", gap: "4px" }}>
                  {patrolSummary.healed_details.map((h, i) => (
                    <div key={i} className="mono" style={{ fontSize: "11px" }}>
                      Hash: {h.hash.slice(0, 16)}... | Replaced using healthy source: <strong>{h.source_path_used}</strong>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {patrolSummary.unrecoverable_details.length > 0 && (
              <div style={{ background: "rgba(239, 68, 68, 0.06)", border: "1px solid rgba(239, 68, 68, 0.3)", borderRadius: "var(--radius)", padding: "12px" }}>
                <h4 style={{ margin: "0 0 6px 0", color: "var(--danger)", display: "flex", alignItems: "center", gap: "6px" }}>
                  <IconAlert size={16} />
                  <span>Unrecoverable Corruptions ({patrolSummary.unrecoverable_details.length})</span>
                </h4>
                <p style={{ margin: "0 0 6px 0", fontSize: "12px", color: "var(--text-dim)" }}>
                  The original source file could not be found to heal these pool objects. Affected snapshots have been flagged as Corrupted.
                </p>
                <div style={{ fontSize: "12px", display: "flex", flexDirection: "column", gap: "4px" }}>
                  {patrolSummary.unrecoverable_details.map((u, i) => (
                    <div key={i} className="mono" style={{ fontSize: "11px", color: "var(--danger)" }}>
                      Object: {u.hash.slice(0, 16)}... | Impacted file: {u.affected_relative_paths.join(", ")}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
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
          <button
            className="btn btn-primary"
            onClick={() => onNavigate && onNavigate("backup-create")}
          >
            Create New Backup
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => onNavigate && onNavigate("backups")}
          >
            View All Backups
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => onNavigate && onNavigate("settings")}
          >
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