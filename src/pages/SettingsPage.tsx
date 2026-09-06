import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { BackupSchedule, RetentionPolicy, VerificationMode } from "../types";

interface Exclusion {
  id: number;
  path: string;
}

export default function SettingsPage() {
  const [defaultLocation, setDefaultLocation] = useState<string>("");
  const [retentionDays, setRetentionDays] = useState(90);
  const [concurrency, setConcurrency] = useState(4);
  const [exclusions, setExclusions] = useState<Exclusion[]>([]);
  const [saved, setSaved] = useState(false);

  // Backup settings
  const [defaultBackupDestination, setDefaultBackupDestination] = useState<string>("");
  const [defaultVerificationMode, setDefaultVerificationMode] = useState<VerificationMode>("standard");
  const [defaultBackupConcurrency, setDefaultBackupConcurrency] = useState(4);
  const [defaultSchedule, setDefaultSchedule] = useState<BackupSchedule>("manual");
  const [defaultRetentionPolicy, setDefaultRetentionPolicy] = useState<RetentionPolicy>("keep_all");

  useEffect(() => {
    api.settings().then((s) => {
      setDefaultLocation(s.default_scan_location ?? "");
      setRetentionDays(s.quarantine_retention_days);
      setConcurrency(s.max_hash_concurrency);
      setDefaultBackupDestination(s.default_backup_destination ?? "");
      setDefaultVerificationMode(s.default_verification_mode);
      setDefaultBackupConcurrency(s.max_backup_concurrency);
      setDefaultSchedule(s.default_backup_schedule);
      setDefaultRetentionPolicy(s.default_retention_policy);
    });
    refreshExclusions();
  }, []);

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
          <span className="hint">Used only as a suggestion — nothing is scanned automatically.</span>
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
        <h3>Quarantine</h3>
        <label className="field">
          Retention reminder after (days): {retentionDays}
          <input
            type="range"
            min={7}
            max={365}
            step={1}
            value={retentionDays}
            onChange={(e) => setRetentionDays(Number(e.target.value))}
          />
          <span className="hint">
            Phase 1 never deletes quarantined files automatically — this only
            controls a future cleanup reminder.
          </span>
        </label>
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