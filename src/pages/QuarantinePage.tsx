import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import type { QuarantineItem } from "../types";
import ConfirmDialog from "../components/ConfirmDialog";

interface Props {
  showToast: (text: string, kind?: "info" | "error") => void;
}

type ConflictChoice = "keep_existing" | "new_name" | "cancel";

export default function QuarantinePage({ showToast }: Props) {
  const [items, setItems] = useState<QuarantineItem[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(true);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [conflicts, setConflicts] = useState<
    { item_id: number; original_path: string }[] | null
  >(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.quarantineItems());
      setSelected(new Set());
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const stored = items.filter((i) => i.status === "stored");

  const startRestore = async (ids: number[]) => {
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const preview = await api.restorePreview(ids);
      const conflicted = preview
        .filter((p) => p.file_exists_at_original)
        .map((p) => ({ item_id: p.item_id, original_path: p.original_path }));
      const clean = ids.filter((id) => !conflicted.some((c) => c.item_id === id));
      let okCount = 0;
      let failCount = 0;
      if (clean.length > 0) {
        const res = await api.restoreExecute(clean);
        okCount += res.report.succeeded;
        failCount += res.report.failed;
      }
      if (conflicted.length > 0) {
        setConflicts(conflicted);
      }
      if (clean.length > 0 && conflicted.length === 0) {
        showToast(
          failCount === 0
            ? `Restored ${okCount} file(s).`
            : `Restored ${okCount}, failed ${failCount}.`,
          failCount === 0 ? "info" : "error"
        );
        await load();
      }
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const resolveConflicts = async (choice: ConflictChoice) => {
    if (!conflicts) return;
    if (choice !== "cancel") {
      try {
        const ids = conflicts.map((c) => c.item_id);
        const res = await api.restoreExecute(ids, choice);
        showToast(
          `Resolved ${res.report.succeeded} restore(s), kept existing ${res.kept_existing.length}, failed ${res.report.failed}.`,
          res.report.failed === 0 ? "info" : "error"
        );
      } catch (e) {
        showToast(String(e), "error");
      }
    }
    setConflicts(null);
    await load();
  };

  const doDelete = async () => {
    if (!confirmDelete) return;
    setBusy(true);
    try {
      const report = await api.deletePermanently([...selected]);
      if (report.failed === 0) {
        showToast(`Permanently deleted ${report.succeeded} file(s).`);
      } else {
        showToast(`Deleted ${report.succeeded}, failed ${report.failed}.`, "error");
      }
      setConfirmDelete(false);
      await load();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const deleteTotal = stored
    .filter((i) => selected.has(i.id))
    .reduce((sum, i) => sum + i.size_bytes, 0);

  if (loading) return <p>Loading quarantine…</p>;

  return (
    <section>
      <div className="page-header">
        <h2>Quarantine</h2>
        <span className="meta">
          {stored.length} file(s) held ·{" "}
          {formatBytes(stored.reduce((s, i) => s + i.size_bytes, 0))}
        </span>
      </div>

      {stored.length === 0 ? (
        <div className="empty-state">
          <h3>Quarantine is empty</h3>
          <p>Files you move here from a scan will appear in this section.</p>
        </div>
      ) : (
        <>
          <div className="selection-bar">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={stored.length > 0 && stored.every((i) => selected.has(i.id))}
                onChange={(e) =>
                  setSelected(e.target.checked ? new Set(stored.map((i) => i.id)) : new Set())
                }
              />
              Select all
            </label>
            <div className="row">
              <button
                className="btn btn-primary"
                disabled={selected.size === 0 || busy}
                onClick={() => startRestore([...selected])}
              >
                Restore selected…
              </button>
              <button
                className="btn btn-danger"
                disabled={selected.size === 0 || busy}
                onClick={() => setConfirmDelete(true)}
              >
                Delete permanently…
              </button>
            </div>
          </div>

          <table className="file-table">
            <thead>
              <tr>
                <th scope="col">Select</th>
                <th scope="col">Original location</th>
                <th scope="col">Size</th>
                <th scope="col">Quarantined</th>
                <th scope="col"></th>
              </tr>
            </thead>
            <tbody>
              {stored.map((item) => (
                <tr key={item.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.has(item.id)}
                      onChange={() =>
                        setSelected((prev) => {
                          const n = new Set(prev);
                          if (n.has(item.id)) n.delete(item.id);
                          else n.add(item.id);
                          return n;
                        })
                      }
                      aria-label={`Select ${item.original_path}`}
                    />
                  </td>
                  <td className="mono truncated" title={item.original_path}>
                    {item.original_path}
                  </td>
                  <td>{formatBytes(item.size_bytes)}</td>
                  <td>{formatDate(item.quarantined_at_ms)}</td>
                  <td>
                    <button
                      className="btn btn-ghost btn-small"
                      disabled={busy}
                      onClick={() => startRestore([item.id])}
                    >
                      Restore
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {confirmDelete && (
        <ConfirmDialog
          title={`Permanently delete ${selected.size} file(s)?`}
          body={
            <>
              <p>
                Total size: <strong>{formatBytes(deleteTotal)}</strong>
              </p>
              <p className="danger-text">
                This action cannot be undone. The files will be removed from the
                quarantine area and will no longer be recoverable.
              </p>
            </>
          }
          confirmLabel="Delete Permanently"
          danger
          onConfirm={doDelete}
          onCancel={() => setConfirmDelete(false)}
        />
      )}

      {conflicts && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true">
          <div className="modal">
            <h2>Restore conflict</h2>
            <div className="modal-body">
              {conflicts.map((c) => (
                <p key={c.item_id} className="mono conflict-path">
                  A file already exists at:
                  <br />
                  {c.original_path}
                </p>
              ))}
              <p>The existing file(s) will never be overwritten automatically.</p>
              <p>Choose how to resolve the conflict for all listed items:</p>
            </div>
            <div className="modal-actions column">
              <button className="btn btn-secondary" onClick={() => resolveConflicts("keep_existing")}>
                Keep Existing — leave the current file in place
              </button>
              <button className="btn btn-primary" onClick={() => resolveConflicts("new_name")}>
                Restore With New Name — save next to it as “name (restored).ext”
              </button>
              <button className="btn btn-ghost" onClick={() => resolveConflicts("cancel")}>
                Cancel — quarantined copy stays in quarantine
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

