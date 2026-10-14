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
  const [hideStructural, setHideStructural] = useState(true);

  const visibleItems = hideStructural
    ? items.filter((f) => f.empty_risk !== 'high')
    : items;
  const hiddenCount = items.filter((f) => f.empty_risk === 'high').length;

  const ignoreFolder = async (filePath: string) => {
    const sep = filePath.includes('\\') ? '\\' : '/';
    const parts = filePath.split(sep);
    parts.pop();
    const folder = parts.join(sep);
    if (!folder) return;
    try {
      await api.addExclusion(folder);
      showToast(`Excluded folder: ${folder}`);
      await load(page);
    } catch (e) {
      showToast(`Failed to exclude folder: ${String(e)}`, 'error');
    }
  };

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

  const allOnPageSelected = visibleItems.length > 0 && visibleItems.every((f) => selected.has(f.id));

  const toggleAll = () => {
    setSelected(allOnPageSelected ? new Set() : new Set(visibleItems.map((f) => f.id)));
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
          <div className="empty-files-filter-bar">
            <label className="checkbox-label" style={{ fontWeight: 600 }}>
              <input
                type="checkbox"
                checked={hideStructural}
                onChange={(e) => setHideStructural(e.target.checked)}
              />
              <span>🛡️ Hide App & Game Structural Files (Recommended)</span>
            </label>
            <span className="meta">
              {hideStructural
                ? `${hiddenCount} structural file(s) protected & hidden · Showing ${visibleItems.length} candidate(s)`
                : `Showing all ${items.length} file(s) (${hiddenCount} sensitive game/config file(s) visible)`}
            </span>
          </div>

          <div className="selection-bar">
            <label className="checkbox-label">
              <input type="checkbox" checked={allOnPageSelected} onChange={toggleAll} />
              Select all visible ({visibleItems.length})
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
                <th scope="col">Safety / Purpose</th>
                <th scope="col">Path</th>
                <th scope="col">Created</th>
                <th scope="col">Modified</th>
              </tr>
            </thead>
            <tbody>
              {visibleItems.map((f) => (
                <tr key={f.id} className={f.empty_risk === 'high' ? 'row-high-risk' : ''}>
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
                  <td>
                    {f.empty_risk === 'high' ? (
                      <span className="badge badge-risk-high" title={f.empty_risk_reason ?? "Structural file required by application/game"}>
                        ⚠️ {f.empty_risk_badge ?? "App/Game Structure"}
                      </span>
                    ) : f.empty_risk === 'medium' ? (
                      <span className="badge badge-risk-medium" title={f.empty_risk_reason ?? "User document"}>
                        ℹ️ {f.empty_risk_badge ?? "User Document"}
                      </span>
                    ) : (
                      <span className="badge badge-risk-low" title={f.empty_risk_reason ?? "Safe to clean"}>
                        ✓ {f.empty_risk_badge ?? "Safe Junk"}
                      </span>
                    )}
                  </td>
                  <td className="mono truncated" title={f.path}>
                    <div className="dup-path-cell">
                      <span className="dup-path-text">{f.path}</span>
                      <button
                        type="button"
                        className="btn-link-action"
                        title="Exclude this folder from empty file scan"
                        onClick={(e) => {
                          e.stopPropagation();
                          ignoreFolder(f.path);
                        }}
                      >
                        Ignore folder
                      </button>
                    </div>
                  </td>
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
              {(() => {
                const highRiskSelected = items.filter((f) => selected.has(f.id) && f.empty_risk === 'high');
                if (highRiskSelected.length === 0) return null;
                return (
                  <div className="notice notice-warning" style={{ marginTop: "12px", textAlign: "left" }}>
                    <strong>⚠️ High-Risk Warning:</strong> {highRiskSelected.length} of the selected file(s) are critical game saves or configuration templates (such as Minecraft .mca region chunks or mod configs). Moving them may corrupt saves or crash mods!
                  </div>
                );
              })()}
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

