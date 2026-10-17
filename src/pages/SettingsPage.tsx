import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { api, formatBytes } from "../api";
import { IconShield, IconLock } from "../components/NavIcons";
import type { BackupSchedule, RetentionPolicy, VerificationMode, PrivacyAuditResult, UpdateCheckResult } from "../types";

interface Exclusion {
  id: number;
  path: string;
}

export default function SettingsPage() {
  const [defaultLocation, setDefaultLocation] = useState<string>("");
  const [retentionDays, setRetentionDays] = useState(90);
  const [concurrency, setConcurrency] = useState(4);
  const [exclusions, setExclusions] = useState<Exclusion[]>([]);
  const [privacyShieldEnabled, setPrivacyShieldEnabled] = useState(true);
  const [privacyAudit, setPrivacyAudit] = useState<PrivacyAuditResult | null>(null);
  const [saved, setSaved] = useState(false);

  // Backup settings
  const [defaultBackupDestination, setDefaultBackupDestination] = useState<string>("");
  const [defaultVerificationMode, setDefaultVerificationMode] = useState<VerificationMode>("standard");
  const [defaultBackupConcurrency, setDefaultBackupConcurrency] = useState(4);
  const [defaultSchedule, setDefaultSchedule] = useState<BackupSchedule>("manual");
  const [defaultRetentionPolicy, setDefaultRetentionPolicy] = useState<RetentionPolicy>("keep_all");

  // Software update state
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [downloadingUpdate, setDownloadingUpdate] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<{ downloaded: number; total: number; percentage: number } | null>(null);

  useEffect(() => {
    api.settings().then((s) => {
      setDefaultLocation(s.default_scan_location ?? "");
      setRetentionDays(s.quarantine_retention_days);
      setConcurrency(s.max_hash_concurrency);
      setPrivacyShieldEnabled(s.privacy_shield_enabled ?? true);
      setDefaultBackupDestination(s.default_backup_destination ?? "");
      setDefaultVerificationMode(s.default_verification_mode);
      setDefaultBackupConcurrency(s.max_backup_concurrency);
      setDefaultSchedule(s.default_backup_schedule);
      setDefaultRetentionPolicy(s.default_retention_policy);
    });
    api.getPrivacyAudit().then(setPrivacyAudit).catch(console.error);
    refreshExclusions();

    const unlistenProgress = listen<{ downloaded: number; total: number; percentage: number }>(
      "update-download-progress",
      (event) => {
        setDownloadProgress(event.payload);
      }
    );
    return () => {
      unlistenProgress.then((f) => f());
    };
  }, []);

  const handleCheckForUpdates = async () => {
    setCheckingUpdate(true);
    setUpdateError(null);
    try {
      const res = await api.checkForUpdates();
      setUpdateInfo(res);
    } catch (err) {
      setUpdateError(String(err));
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleDownloadAndInstall = async () => {
    if (!updateInfo?.asset_url) return;
    setDownloadingUpdate(true);
    setUpdateError(null);
    try {
      await api.downloadAndInstallUpdate(updateInfo.asset_url);
    } catch (err) {
      setUpdateError(String(err));
      setDownloadingUpdate(false);
    }
  };

  const refreshExclusions = () => {
    api.exclusions().then((rows) =>
      setExclusions(rows.map(([id, path]) => ({ id, path })))
    );
  };

  const save = async () => {
    await api.saveSettings({
      default_scan_location: defaultLocation || null,
      quarantine_retention_days: retentionDays,
      max_hash_concurrency: Math.max(1, Math.min(64, concurrency)),
      privacy_shield_enabled: privacyShieldEnabled,
      default_backup_destination: defaultBackupDestination || null,
      default_verification_mode: defaultVerificationMode,
      max_backup_concurrency: Math.max(1, Math.min(64, defaultBackupConcurrency)),
      default_backup_schedule: defaultSchedule,
      default_retention_policy: defaultRetentionPolicy,
    });
    setSaved(true);
    window.setTimeout(() => setSaved(false), 3000);
  };

  const addExclusion = async () => {
    const selected = await open({ directory: true });
    if (typeof selected === "string") {
      await api.addExclusion(selected);
      refreshExclusions();
    }
  };

  return (
    <section>
      <h2>Settings</h2>

      <div className="card">
        <h3>Scanning</h3>
        <label className="field">
          Default scan location
          <div className="row">
            <input
              type="text"
              value={defaultLocation}
              onChange={(e) => setDefaultLocation(e.target.value)}
              placeholder="e.g. C:\Users\you\Downloads"
              className="grow"
            />
            <button
              type="button"
              className="btn btn-secondary"
              onClick={async () => {
                const sel = await open({ directory: true });
                if (typeof sel === "string") setDefaultLocation(sel);
              }}
            >
              Browse…
            </button>
          </div>
          <span className="hint">Used only as a suggestion; nothing is scanned automatically.</span>
        </label>

        <label className="field">
          Hashing workers: {concurrency}
          <input
            type="range"
            min={1}
            max={16}
            value={concurrency}
            onChange={(e) => setConcurrency(Number(e.target.value))}
          />
          <span className="hint">
            More workers hash duplicate candidates faster but use more disk I/O.
          </span>
        </label>
      </div>

      <div className="card">
        <h3>Backup</h3>
        <label className="field">
          Default backup destination
          <div className="row">
            <input
              type="text"
              value={defaultBackupDestination}
              onChange={(e) => setDefaultBackupDestination(e.target.value)}
              placeholder="e.g. D:\FileHarbor Backups"
              className="grow"
            />
            <button
              type="button"
              className="btn btn-secondary"
              onClick={async () => {
                const sel = await open({ directory: true });
                if (typeof sel === "string") setDefaultBackupDestination(sel);
              }}
            >
              Browse…
            </button>
          </div>
          <span className="hint">Default destination for new backup sets.</span>
        </label>

        <label className="field">
          Default verification mode
          <select
            value={defaultVerificationMode}
            onChange={(e) => setDefaultVerificationMode(e.target.value as VerificationMode)}
          >
            <option value="standard">Standard (verify transferred files)</option>
            <option value="deep">Deep (rehash entire backup)</option>
          </select>
          <span className="hint">Standard verifies only new/changed files. Deep rehashes the entire backup.</span>
        </label>

        <label className="field">
          Backup workers: {defaultBackupConcurrency}
          <input
            type="range"
            min={1}
            max={16}
            value={defaultBackupConcurrency}
            onChange={(e) => setDefaultBackupConcurrency(Number(e.target.value))}
          />
          <span className="hint">
            Workers for backup operations. Separate from scanning workers.
          </span>
        </label>

        <label className="field">
          Default schedule
          <select
            value={defaultSchedule}
            onChange={(e) => setDefaultSchedule(e.target.value as BackupSchedule)}
          >
            <option value="manual">Manual only</option>
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
          </select>
          <span className="hint">Automatic backup schedule for new backup sets.</span>
        </label>

        <label className="field">
          Default retention policy
          <select
            value={defaultRetentionPolicy}
            onChange={(e) => setDefaultRetentionPolicy(e.target.value as RetentionPolicy)}
          >
            <option value="keep_all">Keep all snapshots</option>
            <option value="keep_last_10">Keep last 10 snapshots</option>
            <option value="keep_last_50">Keep last 50 snapshots</option>
            <option value="keep_last_100">Keep last 100 snapshots</option>
          </select>
          <span className="hint">Automatically delete older snapshots when this limit is exceeded.</span>
        </label>
      </div>

      <div className="card">
        <h3>Excluded locations</h3>
        <p className="hint">
          These directories are skipped during scans. Common examples:
          node_modules, .git, .cache.
        </p>
        {exclusions.length === 0 ? (
          <p className="empty-inline">No exclusions configured.</p>
        ) : (
          <ul className="path-list">
            {exclusions.map((ex) => (
              <li key={ex.id}>
                <span className="mono">{ex.path}</span>
                <button
                  className="btn btn-ghost btn-small"
                  onClick={async () => {
                    await api.removeExclusion(ex.id);
                    refreshExclusions();
                  }}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
        <button type="button" className="btn btn-secondary" onClick={addExclusion}>
          Add exclusion…
        </button>
      </div>

      <div className="card">
        <h3>Quarantine & Retention Window (Option C)</h3>
        <p className="hint" style={{ marginTop: "4px", marginBottom: "12px" }}>
          FileHarbor utilizes a 30-Day Safe Retention Window (Option C). Files moved during 1-click Auto-Triage or manual deduplication are never permanently deleted automatically. They are held in Quarantine where they remain 100% restorable.
        </p>

        <label className="field">
          Safety Retention Buffer:
          <select
            value={retentionDays}
            onChange={(e) => setRetentionDays(Number(e.target.value))}
            style={{ maxWidth: "340px", marginTop: "4px" }}
          >
            <option value={14}>14 Days (Short Safety Buffer)</option>
            <option value={30}>30 Days (Recommended - Option C Standard)</option>
            <option value={60}>60 Days (Extended Protection)</option>
            <option value={90}>90 Days (Quarterly Audit)</option>
            <option value={180}>180 Days (Half-Year Vault)</option>
          </select>
          <span className="hint" style={{ marginTop: "6px" }}>
            After {retentionDays} days in quarantine, files are flagged with an executive 1-click purge prompt. No permanent deletion occurs without your direct confirmation.
          </span>
        </label>
      </div>

      <div className="card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h3 style={{ margin: 0, display: "flex", alignItems: "center", gap: "8px" }}>
              <IconShield size={18} />
              <span>Privacy & Zero-Knowledge Architecture</span>
            </h3>
            <p className="hint" style={{ marginTop: "4px", marginBottom: "8px" }}>
              FileHarbor is architected to operate with strict local privacy. No data ever leaves your device.
            </p>
          </div>
          <span
            className="badge"
            style={{
              padding: "4px 10px",
              background: "rgba(16, 185, 129, 0.12)",
              color: "var(--success)",
              border: "1px solid rgba(16, 185, 129, 0.3)",
              borderRadius: "12px",
              fontSize: "12px",
              fontWeight: 600,
              display: "inline-flex",
              alignItems: "center",
              gap: "6px",
            }}
          >
            <IconLock size={13} />
            <span>100% Offline / Air-Gapped</span>
          </span>
        </div>

        <div style={{ marginTop: "12px", display: "flex", flexDirection: "column", gap: "10px" }}>
          <label style={{ display: "flex", alignItems: "center", gap: "10px", cursor: "pointer" }}>
            <input
              type="checkbox"
              checked={privacyShieldEnabled}
              onChange={(e) => setPrivacyShieldEnabled(e.target.checked)}
              aria-label="Shield sensitive credentials and key directories"
              style={{ width: "18px", height: "18px" }}
            />
            <div>
              <strong>Shield sensitive credentials and key directories (Recommended)</strong>
              <div className="hint">
                Automatically skips sensitive directories (.ssh, .aws, .gnupg, .kube, crypto wallets) and credential vaults (.kdbx, .pem, .key) during all scans, folder triage, and health inspections.
              </div>
            </div>
          </label>

          <div
            style={{
              background: "var(--bg)",
              border: "1px solid var(--border)",
              padding: "12px",
              borderRadius: "var(--radius-sm)",
              fontSize: "13px",
              display: "flex",
              flexDirection: "column",
              gap: "6px",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)" }}>Network Traffic:</span>
              <strong style={{ color: "var(--success)" }}>0 bytes transmitted (Zero Network Permissions)</strong>
            </div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)" }}>Storage Guarantee:</span>
              <span>Metadata & SHA-256 hashes only (raw file text/previews never cached to disk)</span>
            </div>
            {privacyAudit && (
              <div>
                <span style={{ color: "var(--text-muted)" }}>Shielded directory targets: </span>
                <span className="mono" style={{ fontSize: "11px" }}>
                  {privacyAudit.shielded_directories.join(", ")}
                </span>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h3 style={{ margin: 0 }}>Software Updates</h3>
            <p className="hint" style={{ marginTop: "4px", marginBottom: "0" }}>
              Check GitHub for the latest FileHarbor releases, security improvements, and optimizations.
            </p>
          </div>
          <span
            className="badge"
            style={{
              fontSize: "12px",
              background: "var(--bg-secondary)",
              border: "1px solid var(--border)",
              padding: "4px 8px",
            }}
          >
            Installed: v0.2.0
          </span>
        </div>

        <div style={{ marginTop: "16px", display: "flex", flexDirection: "column", gap: "12px" }}>
          <div className="row" style={{ alignItems: "center", gap: "10px" }}>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={handleCheckForUpdates}
              disabled={checkingUpdate || downloadingUpdate}
            >
              {checkingUpdate ? "Checking GitHub..." : "Check for Updates"}
            </button>
            {updateInfo && !updateInfo.has_update && (
              <span
                style={{
                  color: "var(--success)",
                  fontSize: "13px",
                  fontWeight: 500,
                  display: "inline-flex",
                  alignItems: "center",
                  gap: "4px",
                }}
              >
                ✓ You are using the latest version of FileHarbor ({updateInfo.current_version})
              </span>
            )}
          </div>

          {updateError && (
            <p className="form-error" role="alert" style={{ margin: 0 }}>
              {updateError}
            </p>
          )}

          {updateInfo?.has_update && (
            <div
              style={{
                background: "rgba(59, 130, 246, 0.08)",
                border: "1px solid rgba(59, 130, 246, 0.3)",
                borderRadius: "var(--radius-sm)",
                padding: "16px",
                display: "flex",
                flexDirection: "column",
                gap: "12px",
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "8px" }}>
                <div>
                  <strong style={{ fontSize: "15px", color: "var(--focus)" }}>
                    🚀 New Release Available: {updateInfo.latest_version}
                  </strong>
                  <div className="hint" style={{ marginTop: "2px" }}>
                    {updateInfo.asset_name ? `${updateInfo.asset_name} · ` : ""}
                    {updateInfo.asset_size_bytes ? formatBytes(updateInfo.asset_size_bytes) : ""}
                  </div>
                </div>
                {!downloadingUpdate && (
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={handleDownloadAndInstall}
                    disabled={!updateInfo.asset_url}
                  >
                    Download & Install Update
                  </button>
                )}
              </div>

              {updateInfo.release_notes && (
                <div
                  style={{
                    background: "var(--bg)",
                    border: "1px solid var(--border)",
                    borderRadius: "4px",
                    padding: "10px 12px",
                    maxHeight: "140px",
                    overflowY: "auto",
                    fontSize: "12px",
                    color: "var(--text-muted)",
                    whiteSpace: "pre-wrap",
                  }}
                >
                  <strong style={{ display: "block", marginBottom: "4px", color: "var(--text)" }}>Release Notes:</strong>
                  {updateInfo.release_notes}
                </div>
              )}

              {downloadingUpdate && (
                <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: "12px" }}>
                    <span>Downloading update from GitHub...</span>
                    <span>
                      {downloadProgress
                        ? `${formatBytes(downloadProgress.downloaded)} / ${formatBytes(downloadProgress.total)} (${Math.round(downloadProgress.percentage)}%)`
                        : "Connecting..."}
                    </span>
                  </div>
                  <div
                    style={{
                      height: "8px",
                      background: "var(--border)",
                      borderRadius: "4px",
                      overflow: "hidden",
                    }}
                  >
                    <div
                      style={{
                        height: "100%",
                        width: `${downloadProgress?.percentage ?? 0}%`,
                        background: "var(--primary)",
                      }}
                    />
                  </div>
                  <span className="hint" style={{ fontSize: "11px" }}>
                    Once finished, FileHarbor will launch the update installer automatically.
                  </span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="row">
        <button className="btn btn-primary" onClick={save}>
          Save settings
        </button>
        {saved && (
          <span role="status" className="success-text">
            Saved.
          </span>
        )}
      </div>
    </section>
  );
}