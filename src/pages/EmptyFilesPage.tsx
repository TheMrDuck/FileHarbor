import { useCallback, useEffect, useState } from "react";
import { api, formatDate } from "../api";
import type { FileRecord } from "../types";

interface Props {
  sessionId: number;
  showToast: (text: string, kind?: "info" | "error") => void;
}

const PAGE_SIZE = 100;

export default function EmptyFilesPage({ sessionId, showToast }: Props) {
  const [items, setItems] = useState<FileRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(true);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async (p: number) => {
    setLoading(true);
    try {
      const res = await api.listFiles({
        session_id: sessionId,
        empty_only: true,
        page: p,
        page_size: PAGE_SIZE,
        sort: "path",
        ascending: true,
      });
      setItems(res.items);
      setTotal(res.total);
      setPage(p);
      setSelected(new Set());
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    load(0);
  }, [load]);

  const allOnPageSelected = items.length > 0 && items.every((f) => selected.has(f.id));

  const toggleAll = () => {
    setSelected(allOnPageSelected ? new Set() : new Set(items.map((f) => f.id)));
  };

  const doQuarantine = async () => {
    setBusy(true);
    try {
      const { report } = await api.quarantineFiles([...selected]);
      if (report.failed === 0) {
        showToast(`Moved ${report.succeeded} empty file(s) to quarantine.`);
      } else {
        showToast(`Moved ${report.succeeded}, failed ${report.failed}.`, "error");
      }
      setConfirming(false);
      await load(page);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  if (loading) return <p>Loading empty files…</p>;

  return (
    <section>
      <div className="page-header">
        <h2>Empty Files</h2>
        <span className="meta">{total.toLocaleString()} file(s)</span>
      </div>

      <div className="notice notice-info">
        Empty does not mean useless. Zero-byte files are sometimes used as flags,
        lock markers or placeholders (.gitkeep and similar). Review before moving
        anything. Nothing happens without your confirmation.
      </div>

      {total === 0 ? (
        <div className="empty-state">
          <h3>No empty files found</h3>
          <p>Every scanned file contains data.</p>
        </div>
      ) : (
        <>
          <div className="selection-bar">
            <label className="checkbox-label">
              <input type="checkbox" checked={allOnPageSelected} onChange={toggleAll} />
              Select all on page ({items.length})
            </label>
            <button
              className="btn btn-primary"
              disabled={selected.size === 0}
              onClick={() => setConfirming(true)}
            >
              Move {selected.size || ""} selected to quarantine…
            </button>
          </div>

          <table className="file-table">
            <thead>
              <tr>
                <th scope="col">Select</th>
                <th scope="col">File</th>
                <th scope="col">Path</th>
                <th scope="col">Created</th>
                <th scope="col">Modified</th>
              </tr>
            </thead>
            <tbody>
              {items.map((f) => (
                <tr key={f.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.has(f.id)}
                      onChange={() =>
                        setSelected((prev) => {
                          const n = new Set(prev);
                          if (n.has(f.id)) n.delete(f.id);
                          else n.add(f.id);
                          return n;
                        })
                      }
                      aria-label={`Select ${f.path}`}
                    />
                  </td>
                  <td className="mono">{f.file_name}</td>
                  <td className="mono truncated" title={f.path}>{f.path}</td>
                  <td>{formatDate(f.created_at_ms)}</td>
                  <td>{formatDate(f.modified_at_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>

          <nav className="pager" aria-label="Empty files pages">
            <button className="btn btn-secondary" disabled={page === 0} onClick={() => load(page - 1)}>
              Previous
            </button>
            <button
              className="btn btn-secondary"
              disabled={(page + 1) * PAGE_SIZE >= total}
              onClick={() => load(page + 1)}
            >
              Next
            </button>
          </nav>
        </>
      )}

      {confirming && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true">
          <div className="modal">
            <h2>You selected:</h2>
            <div className="modal-body">
              <p>
                <strong>{selected.size}</strong> empty file(s)
              </p>
              <p>They will be moved to Quarantine (not deleted).</p>
            </div>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setConfirming(false)} disabled={busy}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={doQuarantine} disabled={busy}>
                {busy ? "Moving…" : "Move to Quarantine"}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

