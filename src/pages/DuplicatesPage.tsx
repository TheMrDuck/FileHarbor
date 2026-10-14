import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import { IconShield, IconLightbulb, IconAlert, NavIcon } from "../components/NavIcons";
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
  const [activeExplanation, setActiveExplanation] = useState<import("../types").DuplicateExplanation | null>(null);
  const [loadingExplanation, setLoadingExplanation] = useState<boolean>(false);

  const handleOpenWhy = async (groupId: number) => {
    setLoadingExplanation(true);
    try {
      const exp = await api.getDuplicateExplanation(groupId);
      setActiveExplanation(exp);
    } catch (err) {
      showToast(`Failed to load explanation: ${String(err)}`, "error");
    } finally {
      setLoadingExplanation(false);
    }
  };

  const PAGE_SIZE = 25;

  const load = useCallback(
    async (p: number) => {
      setLoading(true);
      try {
        const res = await api.duplicateGroups(sessionId, p);
        setGroups(res.items);
        setTotalGroups(res.total);
        // Safe default selection:
        // If files are cross-project (e.g. server.jar across servers) or internal VCS,
        // leave them unselected by default to prevent breaking working instances!
        const next = new Map<number, Set<number>>();
        for (const g of res.items) {
          if (g.is_cross_project) {
            next.set(g.id, new Set());
          } else {
            next.set(
              g.id,
              new Set(
                g.members
                  .filter((m) => m.id !== g.recommended_keep_file_id && m.copy_role !== 'backup' && !m.is_vcs_internal)
                  .map((m) => m.id)
              )
            );
          }
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
    const group = groups.find((g) => g.id === groupId);
    const member = group?.members.find((m) => m.id === fileId);
    if (member?.copy_role === "backup") return;
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
        new Set(g.members.filter((m) => m.id !== g.recommended_keep_file_id && m.copy_role !== 'backup' && !m.is_vcs_internal).map((m) => m.id))
      );
      return next;
    });

  const keepAllInGroup = (groupId: number) => {
    setSelected((prev) => {
      const next = new Map(prev);
      next.set(groupId, new Set());
      return next;
    });
    showToast('Marked all copies in this group to keep.');
  };

  const deselectAllOnPage = () => {
    const next = new Map<number, Set<number>>();
    for (const g of groups) {
      next.set(g.id, new Set());
    }
    setSelected(next);
    showToast('Cleared all selections on this page.');
  };

  const ignoreFolder = async (filePath: string) => {
    const sep = filePath.includes('\\') ? '\\' : '/';
    const parts = filePath.split(sep);
    parts.pop(); // remove file name
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

  const selectedFiles = () => {
    const ids: number[] = [];
    let bytes = 0;
    for (const g of groups) {
      for (const id of selected.get(g.id) ?? []) {
        const member = g.members.find((m) => m.id === id);
        if (member && member.copy_role !== "backup") {
          ids.push(id);
          bytes += g.size_bytes;
        }
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
          `Moved ${report.succeeded}, failed ${report.failed}. Failed files stayed in place (see Operation History).`,
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
          <p>Every scanned file currently has unique content.</p>
        </div>
      ) : (
        <>
          <div className="selection-bar" role="status">
            <span>
              <strong>{selIds.length}</strong> file(s) selected ·{" "}
              <strong>{formatBytes(selBytes)}</strong> will move to Quarantine
            </span>
            <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
              <button
                className="btn btn-secondary btn-small"
                onClick={deselectAllOnPage}
                disabled={selIds.length === 0}
                title="Deselect all files on this page to keep everything"
              >
                Deselect All
              </button>
              <button
                className="btn btn-primary"
                disabled={selIds.length === 0}
                onClick={() => setConfirming({ files: selIds.length, bytes: selBytes, ids: selIds })}
              >
                Review & move to quarantine…
              </button>
            </div>
          </div>

          {groups.map((g) => {
            const keep = g.members.find((m) => m.id === g.recommended_keep_file_id);
            const groupSel = selected.get(g.id) ?? new Set<number>();
            return (
              <article key={g.id} className="card dup-group">
                <header className="dup-header">
                  <div style={{ display: "flex", alignItems: "center", gap: "10px", flexWrap: "wrap" }}>
                    <h3>
                      {g.members.length} identical files · {formatBytes(g.size_bytes)} each
                    </h3>
                    {g.is_cross_project && (
                      <span className="badge badge-cross-project" title="Files reside in separate project/server roots">
                        ⚠️ Cross-Project Runtimes
                      </span>
                    )}
                  </div>
                  <span className="reclaimable">
                    Reclaimable: <strong>{formatBytes(groupReclaimable(g))}</strong>
                  </span>
                </header>

                {g.is_cross_project && (
                  <div className="cross-project-banner">
                    <IconAlert size={14} />
                    <span>
                      <strong>Independent Server / Project Roots:</strong> These files are located in separate server environments (e.g. independent Minecraft servers). Kept by default to prevent breaking running servers.
                    </span>
                  </div>
                )}

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
                    {g.members.map((m) => {
                      const isKeep = m.id === g.recommended_keep_file_id;
                      const isBackup = m.copy_role === "backup";
                      return (
                        <tr
                          key={m.id}
                          className={isKeep ? "keep-row" : isBackup ? "backup-row" : ""}
                        >
                          <td>
                            {isKeep ? (
                              <span className="keep-badge">Keep (recommended)</span>
                            ) : isBackup ? (
                              <span
                                className="badge backup-protected"
                                title="Protected backup copy (cannot be selected for quarantine)"
                              >
                                <IconShield size={12} /> Backup Copy (Protected)
                              </span>
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
                            <div className="dup-path-cell">
                              <span className="dup-path-text">{m.path}</span>
                              <div className="dup-path-tags">
                                {m.project_context && (
                                  <span className="badge badge-project" title={`Project Root: ${m.project_context}`}>
                                    {m.project_context.toLowerCase().includes("server") ? "🎮 " : "📦 "}
                                    {m.project_context}
                                  </span>
                                )}
                                {m.is_vcs_internal && (
                                  <span className="badge badge-vcs" title="Internal Git/VCS storage (do not delete independently)">
                                    🔒 Git Internal
                                  </span>
                                )}
                                <button
                                  type="button"
                                  className="btn-link-action"
                                  title="Whitelist this folder from current and future scans"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    ignoreFolder(m.path);
                                  }}
                                >
                                  Ignore folder
                                </button>
                              </div>
                            </div>
                          </td>
                          <td>{formatBytes(m.size_bytes)}</td>
                          <td>{formatDate(m.modified_at_ms)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>

                <footer className="dup-footer">
                  {keep && (
                    <span className="meta">
                      Recommended to keep: <span className="mono">{keep.file_name}</span>{" "}
                      (original-looking name, oldest copy; you can always choose differently)
                    </span>
                  )}
                  <div className="row">
                    <button
                      className="btn btn-secondary btn-small"
                      style={{ fontWeight: 600, color: "var(--accent)", display: "inline-flex", alignItems: "center", gap: "6px" }}
                      onClick={() => handleOpenWhy(g.id)}
                      disabled={loadingExplanation}
                    >
                      <IconLightbulb size={13} />
                      <span>Why?</span>
                    </button>
                    <button
                      className="btn btn-secondary btn-small"
                      style={{ fontWeight: 600 }}
                      onClick={() => keepAllInGroup(g.id)}
                      title="Keep all copies in this group intact without deleting any"
                    >
                      Keep All (Don't Delete)
                    </button>
                    <button className="btn btn-ghost btn-small" onClick={() => selectAllCopiesExceptKeep(g)}>
                      Reset selection
                    </button>
                    {groupSel.size === 0 && (
                      <span className="badge badge-success" style={{ marginLeft: "auto" }}>
                        ✓ Keeping All Copies
                      </span>
                    )}
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

      {/* "Why?" Explanatory Evidence Modal */}
      {activeExplanation && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true">
          <div className="modal" style={{ maxWidth: "680px", width: "95%" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "16px" }}>
              <h2 style={{ margin: 0, display: "flex", alignItems: "center", gap: "8px" }}>
                <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                  <IconLightbulb size={20} />
                  <span>Duplicate Explanation</span>
                </span>
                <span className="badge" style={{ background: "rgba(16, 185, 129, 0.15)", color: "var(--success)", border: "1px solid var(--success)" }}>
                  {activeExplanation.confidence_percentage}% Confidence
                </span>
              </h2>
              <button
                className="btn btn-ghost btn-small"
                onClick={() => setActiveExplanation(null)}
                aria-label="Close duplicate explanation modal"
                style={{ fontSize: "16px" }}
              >
                ✕
              </button>
            </div>

            <div className="modal-body" style={{ maxHeight: "70vh", overflowY: "auto" }}>
              {/* Detection Level Banner */}
              <div
                style={{
                  background: "rgba(56, 189, 248, 0.1)",
                  border: "1px solid rgba(56, 189, 248, 0.3)",
                  borderRadius: "var(--radius)",
                  padding: "12px 16px",
                  marginBottom: "16px",
                }}
              >
                <div style={{ fontWeight: 600, color: "var(--accent)", marginBottom: "4px", display: "flex", alignItems: "center", gap: "6px" }}>
                  <NavIcon page="scan" />
                  <span>Detection Classification: {activeExplanation.match_level}</span>
                </div>
                <div style={{ fontSize: "13px", color: "var(--text)" }}>
                  All <strong>{activeExplanation.total_members} files</strong> share an identical byte size of{" "}
                  <strong>{formatBytes(activeExplanation.size_bytes)}</strong> and the exact cryptographic SHA-256 hash:
                </div>
                <div
                  className="mono"
                  style={{
                    marginTop: "6px",
                    background: "rgba(0, 0, 0, 0.25)",
                    padding: "6px 8px",
                    borderRadius: "4px",
                    fontSize: "11px",
                    wordBreak: "break-all",
                  }}
                >
                  {activeExplanation.hash}
                </div>
              </div>

              {/* Keep Recommendation Reasoning */}
              <div
                style={{
                  background: "rgba(16, 185, 129, 0.08)",
                  border: "1px solid rgba(16, 185, 129, 0.3)",
                  borderRadius: "var(--radius)",
                  padding: "12px 16px",
                  marginBottom: "16px",
                }}
              >
                <div style={{ fontWeight: 600, color: "var(--success)", marginBottom: "4px", display: "flex", alignItems: "center", gap: "6px" }}>
                  <IconShield size={16} />
                  <span>Why is one file recommended to keep?</span>
                </div>
                <div style={{ fontSize: "13px", color: "var(--text)" }}>
                  {activeExplanation.keep_recommendation_reason}
                </div>
              </div>

              {/* Members Lineage Breakdown */}
              <div>
                <h4 style={{ margin: "0 0 10px 0" }}>Member Lineage & Protection Analysis:</h4>
                <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                  {activeExplanation.member_details.map((m) => (
                    <div
                      key={m.file_id}
                      style={{
                        padding: "10px 12px",
                        borderRadius: "var(--radius)",
                        border: m.is_recommended_keep
                          ? "1px solid rgba(34, 197, 94, 0.4)"
                          : "1px solid var(--border)",
                        background: m.is_recommended_keep
                          ? "rgba(34, 197, 94, 0.04)"
                          : "rgba(255, 255, 255, 0.02)",
                      }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "4px" }}>
                        <span style={{ fontWeight: 600, fontSize: "13px" }}>{m.file_name}</span>
                        <div style={{ display: "flex", gap: "6px" }}>
                          {m.is_recommended_keep && (
                            <span className="keep-badge" style={{ fontSize: "11px" }}>Keep Candidate</span>
                          )}
                          {m.role_protection_note && (
                            <span className="badge backup-protected" style={{ fontSize: "11px" }}>
                              {m.role_protection_note}
                            </span>
                          )}
                        </div>
                      </div>
                      <div className="mono" style={{ fontSize: "11px", color: "var(--text-dim)", wordBreak: "break-all" }}>
                        {m.path}
                      </div>
                      <div style={{ marginTop: "6px", fontSize: "12px", color: "var(--text-dim)", display: "flex", gap: "16px" }}>
                        <span>Modified: {formatDate(m.modified_at_ms)}</span>
                        <span>Folder Depth: {m.path_depth}</span>
                        {m.has_duplicate_markers && (
                          <span style={{ color: "var(--warning)", display: "inline-flex", alignItems: "center", gap: "4px" }}>
                            <IconAlert size={12} />
                            <span>Has Copy/Backup Marker in Name</span>
                          </span>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <div className="modal-actions" style={{ marginTop: "16px" }}>
              <button className="btn btn-primary" onClick={() => setActiveExplanation(null)}>
                Understood
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
