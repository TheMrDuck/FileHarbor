import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import type { DuplicateGroup } from "../types";
import { groupReclaimable } from "../types";

interface Props {
  sessionId: number;
  showToast: (text: string, kind?: "info" | "error") => void;
}

export default function DuplicatesPage({ sessionId, showToast }: Props) {
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [totalGroups, setTotalGroups] = useState(0);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Map<number, Set<number>>>(new Map());
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [confirming, setConfirming] = useState<null | {
    files: number;
    bytes: number;
    ids: number[];
  }>(null);
  const [busy, setBusy] = useState(false);

  const PAGE_SIZE = 25;

  const load = useCallback(
    async (p: number) => {
      setLoading(true);
      try {
        const res = await api.duplicateGroups(sessionId, p);
        setGroups(res.items);
        setTotalGroups(res.total);
        // Default selection: every copy except the recommended keep.
        const next = new Map<number, Set<number>>();
        for (const g of res.items) {
          next.set(
            g.id,
            new Set(g.members.filter((m) => m.id !== g.recommended_keep_file_id).map((m) => m.id))
          );
        }
        setSelected(next);
        setPage(p);
      } finally {
        setLoading(false);
      }
    },
    [sessionId]
  );

  useEffect(() => {
    load(0);
  }, [load]);

  const toggleFile = (groupId: number, fileId: number) => {
    setSelected((prev) => {
      const next = new Map(prev);
      const set = new Set(next.get(groupId) ?? []);
      if (set.has(fileId)) set.delete(fileId);
      else set.add(fileId);
      next.set(groupId, set);
      return next;
    });
  };

  const selectAllCopiesExceptKeep = (g: DuplicateGroup) =>
    setSelected((prev) => {
      const next = new Map(prev);
      next.set(
        g.id,
        new Set(g.members.filter((m) => m.id !== g.recommended_keep_file_id).map((m) => m.id))
      );
      return next;
    });

  const selectedFiles = () => {
    const ids: number[] = [];
    let bytes = 0;
    for (const g of groups) {
      for (const id of selected.get(g.id) ?? []) {
        ids.push(id);
        bytes += g.size_bytes;
      }
    }
    return { ids, bytes };
  };

  const { ids: selIds, bytes: selBytes } = selectedFiles();

  const doQuarantine = async () => {
    if (!confirming) return;
    setBusy(true);
    try {
      const { report } = await api.quarantineFiles(confirming.ids);
      if (report.failed === 0) {
        showToast(`Moved ${report.succeeded} file(s) to quarantine.`);
      } else {
        showToast(
          `Moved ${report.succeeded}, failed ${report.failed}. Failed files stayed in place — see Operation History.`,
          "error"
        );
      }
      setConfirming(null);
      await load(page);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  if (loading) return <p>Loading duplicate groups…</p>;

  return (
    <section>
      <div className="page-header">
        <h2>Duplicates</h2>
        <span className="meta">
          {totalGroups} group{totalGroups === 1 ? "" : "s"} · page {page + 1} of{" "}
          {Math.max(1, Math.ceil(totalGroups / PAGE_SIZE))}
        </span>
      </div>

      {totalGroups === 0 ? (
        <div className="empty-state">
          <h3>No duplicates found</h3>
          <p>Great — every scanned file currently has unique content.</p>
        </div>
      ) : (
        <>
          <div className="selection-bar" role="status">
            <span>
              <strong>{selIds.length}</strong> file(s) selected ·{" "}
              <strong>{formatBytes(selBytes)}</strong> will move to Quarantine
            </span>
            <button
              className="btn btn-primary"
              disabled={selIds.length === 0}
              onClick={() => setConfirming({ files: selIds.length, bytes: selBytes, ids: selIds })}
            >
              Review & move to quarantine…
            </button>
          </div>

          {groups.map((g) => {
            const keep = g.members.find((m) => m.id === g.recommended_keep_file_id);
            const groupSel = selected.get(g.id) ?? new Set<number>();
            return (
              <article key={g.id} className="card dup-group">
                <header className="dup-header">
                  <h3>
                    {g.members.length} identical files · {formatBytes(g.size_bytes)} each
                  </h3>
                  <span className="reclaimable">
                    Reclaimable: <strong>{formatBytes(groupReclaimable(g))}</strong>
                  </span>
                </header>

                <table className="file-table">
                  <thead>
                    <tr>
                      <th scope="col">Move to quarantine</th>
                      <th scope="col">File</th>
                      <th scope="col">Size</th>
                      <th scope="col">Modified</th>
                    </tr>
                  </thead>
                  <tbody>
                    {g.members.map((m) => (
                      <tr key={m.id} className={m.id === g.recommended_keep_file_id ? "keep-row" : ""}>
                        <td>
                          {m.id === g.recommended_keep_file_id ? (
                            <span className="keep-badge">Keep (recommended)</span>
                          ) : (
                            <label className="checkbox-label">
                              <input
                                type="checkbox"
                                checked={groupSel.has(m.id)}
                                onChange={() => toggleFile(g.id, m.id)}
                              />
                              <span className="sr-only">Select {m.path}</span>
                            </label>
                          )}
                        </td>
                        <td className="mono truncated" title={m.path}>
                          {m.path}
                        </td>
                        <td>{formatBytes(m.size_bytes)}</td>
                        <td>{formatDate(m.modified_at_ms)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>

                <footer className="dup-footer">
                  {keep && (
                    <span className="meta">
                      Recommended to keep: <span className="mono">{keep.file_name}</span>{" "}
                      (original-looking name, oldest copy — you can always choose differently)
                    </span>
                  )}
                  <div className="row">
                    <button className="btn btn-ghost btn-small" onClick={() => selectAllCopiesExceptKeep(g)}>
                      Reset selection
                    </button>
                    <button
                      className="btn btn-ghost btn-small"
                      onClick={() =>
                        setExpanded((prev) => {
                          const n = new Set(prev);
                          if (n.has(g.id)) n.delete(g.id);
                          else n.add(g.id);
                          return n;
                        })
                      }
                    >
                      {expanded.has(g.id) ? "Hide details" : "Details"}
                    </button>
                  </div>
                </footer>

                {expanded.has(g.id) && (
                  <dl className="tech-details mono">
                    <dt>SHA-256</dt>
                    <dd className="hash">{g.hash}</dd>
                    <dt>Group ID</dt>
                    <dd>{g.id}</dd>
                  </dl>
                )}
              </article>
            );
          })}

          <nav className="pager" aria-label="Duplicate group pages">
            <button className="btn btn-secondary" disabled={page === 0} onClick={() => load(page - 1)}>
              Previous
            </button>
            <button
              className="btn btn-secondary"
              disabled={(page + 1) * PAGE_SIZE >= totalGroups}
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
                <strong>{confirming.files}</strong> file(s)
                <br />
                <strong>{formatBytes(confirming.bytes)}</strong>
              </p>
              <p>
                These files will be moved to the application <strong>Quarantine</strong>.
                Nothing is deleted. You can restore them at any time.
              </p>
            </div>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setConfirming(null)} disabled={busy}>
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
