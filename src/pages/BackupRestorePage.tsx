import { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { api } from "../api";
import type { BackupSnapshot, BackupRestorePreview, RestoreConflictPolicy } from "../types";
import { formatBytes, formatDate } from "../api";

interface Props {
  snapshotId?: number;
  onNavigate?: (page: string, opts?: { setId?: number }) => void;
}

export default function BackupRestorePage({ snapshotId: propSnapshotId, onNavigate }: Props) {
  const { snapshotId } = useParams<{ snapshotId: string }>();
  const snapshotIdNum = propSnapshotId ?? parseInt(snapshotId || "0", 10);
  const [snapshot, setSnapshot] = useState<BackupSnapshot | null>(null);
  const [preview, setPreview] = useState<BackupRestorePreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [conflicts, setConflicts] = useState<Record<string, RestoreConflictPolicy>>({});

  useEffect(() => {
    loadSnapshot();
  }, [snapshotIdNum]);

  const loadSnapshot = async () => {
    setLoading(true);
    setError(null);
    try {
      // Get snapshot details
      const snapshots = await api.getBackupSnapshots(0); // We need to get the backup set ID from the snapshot
      const snapshot = snapshots.find(s => s.id === snapshotIdNum);
      if (!snapshot) {
        throw new Error("Snapshot not found");
      }
      setSnapshot(snapshot);

      // Get restore preview
      const preview = await api.getRestorePreview(snapshotIdNum, ""); // Destination root will be empty for now
      setPreview(preview);

      // Initialize conflicts with Cancel policy
      const initialConflicts: Record<string, RestoreConflictPolicy> = {};
      preview.conflicts.forEach(c => {
        initialConflicts[c.relative_path] = "cancel";
      });
      setConflicts(initialConflicts);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleConflictChange = (relativePath: string, policy: RestoreConflictPolicy) => {
    setConflicts(prev => ({ ...prev, [relativePath]: policy }));
  };

  const handleRestore = async () => {
    if (!snapshot) return;
    setRestoring(true);
    setError(null);
    try {
      const conflictsArray = Object.entries(conflicts).map(([relative_path, policy]) => ({ relative_path, policy }));
      const result = await api.restoreFromBackup({
        snapshot_id: snapshotIdNum,
        destination_root: "", // Use original location
        conflicts: conflictsArray,
      });
      if (result.failed > 0) {
        setError(`Restore completed with ${result.failed} failures. ${result.restored} files restored.`);
      } else {
        setError(null);
        alert(`Restore completed successfully! ${result.restored} files restored, ${result.verified} verified.`);
        if (onNavigate) onNavigate("backups");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setRestoring(false);
    }
  };

  if (loading) {
    return <section><p>Loading snapshot…</p></section>;
  }

  if (error && !restoring) {
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

  if (!snapshot || !preview) {
    return <section><p>Snapshot not found</p></section>;
  }

  const hasConflicts = preview.conflicts.length > 0;

  return (
    <section className="backup-restore">
      <header className="page-header">
        <h2>Restore from Backup</h2>
        <button
          className="btn btn-ghost"
          onClick={() => onNavigate && onNavigate("backups")}
        >
          Back
        </button>
      </header>

      {error && <p className="form-error" role="alert">{error}</p>}

      <div className="card">
        <h3>Snapshot: {snapshot.label}</h3>
        <dl className="details-list">
          <dt>Created</dt><dd>{formatDate(snapshot.created_at_ms)}</dd>
          <dt>Files</dt><dd className="mono">{snapshot.files_count.toLocaleString()}</dd>
          <dt>Size</dt><dd className="mono">{formatBytes(snapshot.total_size_bytes)}</dd>
          <dt>Status</dt><dd><span className={`badge status-${snapshot.status}`}>{snapshot.status}</span></dd>
        </dl>
      </div>

      {hasConflicts && (
        <div className="card">
          <h3>Conflicts Detected</h3>
          <p className="page-intro">
            {preview.conflicts.length} file(s) already exist at the restore destination. Choose how to handle each conflict.
          </p>
          <table>
            <thead>
              <tr>
                <th scope="col">File</th>
                <th scope="col">Existing Size</th>
                <th scope="col">Backup Size</th>
                <th scope="col">Action</th>
              </tr>
            </thead>
            <tbody>
              {preview.conflicts.map((conflict) => (
                <tr key={conflict.relative_path}>
                  <td className="truncated mono">{conflict.relative_path}</td>
                  <td className="mono">{formatBytes(conflict.existing_size)}</td>
                  <td className="mono">{formatBytes(conflict.backup_size)}</td>
                  <td>
                    <select
                      value={conflicts[conflict.relative_path] || "cancel"}
                      onChange={(e) => handleConflictChange(conflict.relative_path, e.target.value as RestoreConflictPolicy)}
                    >
                      <option value="cancel">Cancel (skip)</option>
                      <option value="keep_existing">Keep Existing</option>
                      <option value="restore_with_new_name">Restore with New Name</option>
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="card">
        <h3>Restore Summary</h3>
        <dl className="details-list">
          <dt>Total Files</dt><dd className="mono">{preview.total_files.toLocaleString()}</dd>
          <dt>Files to Restore</dt><dd className="mono">{preview.missing_files.toLocaleString()}</dd>
          <dt>Existing Files (Conflicts)</dt><dd className="mono">{preview.existing_files.toLocaleString()}</dd>
        </dl>
      </div>

      <div className="wizard-actions">
        <button
          className="btn btn-secondary"
          onClick={() => onNavigate && onNavigate("backups")}
        >
          Cancel
        </button>
        <button
          className="btn btn-primary"
          onClick={handleRestore}
          disabled={restoring}
        >
          {restoring ? "Restoring…" : "Start Restore"}
        </button>
      </div>
    </section>
  );
}