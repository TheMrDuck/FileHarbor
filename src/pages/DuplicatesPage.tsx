import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import { IconShield, IconLightbulb, IconAlert, NavIcon } from "../components/NavIcons";
import type { DuplicateGroup, AutoTriageSummary } from "../types";
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
  const [activeScope, setActiveScope] = useState<string>("all");
  const [triageSummary, setTriageSummary] = useState<AutoTriageSummary | null>(null);
  const [confirmingAutoTriage, setConfirmingAutoTriage] = useState(false);
  const [globalSelection, setGlobalSelection] = useState(false);
  const [busyMergeId, setBusyMergeId] = useState<number | null>(null);


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
    async (p: number, scope = activeScope) => {
      setLoading(true);
      try {
        const [res, summary] = await Promise.all([
          api.duplicateGroups(sessionId, p, scope === "all" ? undefined : scope),
          api.getAutoTriageSummary(sessionId).catch(() => null),
        ]);
        setGroups(res.items);
        setTotalGroups(res.total);
        if (summary) setTriageSummary(summary);
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
    [sessionId, activeScope]
  );

  useEffect(() => {
    load(0, "all");
  }, [sessionId]);

  const handleApproveAutoTriage = async () => {
    setBusy(true);
    try {
      const { report } = await api.applyAutoTriageQuarantine(sessionId, true, true, true);
      if (report.failed === 0) {
        showToast(`Successfully moved ${report.succeeded} duplicate files to Quarantine. 30-day retention active.`);
      } else {
        showToast(`Moved ${report.succeeded}, failed ${report.failed}. Failed files remained in place.`, "error");
      }
      setConfirmingAutoTriage(false);
      setSelected(new Map());
      await load(0, activeScope);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const handleHardlinkMerge = async (groupId: number) => {
    setBusyMergeId(groupId);
    try {
      const report = await api.hardlinkMergeGroup(groupId);
      if (report.merged_paths.length > 0) {
        showToast(`Merged ${report.merged_paths.length} copy(s) into hardlinks! Reclaimed ${formatBytes(report.bytes_saved)}.`);
      }
      if (report.failed_paths.length > 0) {
        showToast(`Notice: ${report.failed_paths.length} file(s) could not be hardlinked (likely on another drive).`, "error");
      }
      await load(page, activeScope);
    } catch (e) {
      showToast(`Merge failed: ${String(e)}`, "error");
    } finally {
      setBusyMergeId(null);
    }
  };

  const handleWhitelistCaches = async () => {
    const commonCaches = ["Library/PackageCache", "node_modules", ".git", "target/release", "target/debug"];
    try {
      for (const c of commonCaches) {
        await api.addExclusion(c);
      }
      showToast("Added development cache patterns to exclusion list.");
      await load(0, activeScope);
    } catch (e) {
      showToast(String(e), "error");
    }
  };


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
      setSelected(new Map());
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

      {triageSummary && triageSummary.total_groups > 0 && (
        <div
          className="card auto-triage-hub"
          style={{
            background: "linear-gradient(135deg, rgba(30, 41, 59, 0.7) 0%, rgba(15, 23, 42, 0.8) 100%)",
            border: "1px solid rgba(59, 130, 246, 0.3)",
            borderRadius: "12px",
            padding: "20px",
            marginBottom: "20px",
            boxShadow: "0 8px 24px rgba(0, 0, 0, 0.25)",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: "12px" }}>
            <div>
              <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                <span style={{ fontSize: "20px" }}>⚡</span>
                <h3 style={{ margin: 0, fontSize: "18px", fontWeight: 700, color: "var(--text-bright)" }}>
                  Executive 1-Click Auto-Triage
                </h3>
                <span className="badge badge-accent" style={{ background: "rgba(59, 130, 246, 0.2)", color: "#60a5fa" }}>
                  High-Scale Automation
                </span>
              </div>
              <p style={{ margin: "8px 0 16px 0", color: "var(--text-dim)", fontSize: "14px" }}>
                FileHarbor analyzed all <strong>{triageSummary.total_groups.toLocaleString()}</strong> duplicate groups. 
                You don't need to manually review {Math.max(1, Math.ceil(triageSummary.total_groups / PAGE_SIZE))} pages — 
                approve the safe triage plan with 1 click.
              </p>
            </div>
            <div style={{ display: "flex", gap: "10px", alignItems: "center", flexWrap: "wrap" }}>
              <button
                className="btn btn-primary"
                style={{ padding: "10px 18px", fontSize: "14px", fontWeight: 700, boxShadow: "0 0 15px rgba(59, 130, 246, 0.35)" }}
                onClick={() => setConfirmingAutoTriage(true)}
                disabled={busy || triageSummary.recommended_quarantine_count === 0}
              >
                ✅ Approve & Quarantine All ({formatBytes(triageSummary.total_reclaimable_bytes)})
              </button>
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "12px", marginTop: "12px" }}>
            <div style={{ background: "rgba(15, 23, 42, 0.6)", padding: "12px 16px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div style={{ fontSize: "12px", color: "var(--text-dim)", textTransform: "uppercase", letterSpacing: "0.5px" }}>Originals Preserved</div>
              <div style={{ fontSize: "20px", fontWeight: 700, color: "#34d399", marginTop: "4px" }}>
                {triageSummary.recommended_keep_count.toLocaleString()} files
              </div>
              <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "2px" }}>Canonical copies kept</div>
            </div>

            <div style={{ background: "rgba(15, 23, 42, 0.6)", padding: "12px 16px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div style={{ fontSize: "12px", color: "var(--text-dim)", textTransform: "uppercase", letterSpacing: "0.5px" }}>To Quarantine</div>
              <div style={{ fontSize: "20px", fontWeight: 700, color: "#60a5fa", marginTop: "4px" }}>
                {triageSummary.recommended_quarantine_count.toLocaleString()} files
              </div>
              <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "2px" }}>{formatBytes(triageSummary.total_reclaimable_bytes)} reclaimable</div>
            </div>

            <div style={{ background: "rgba(15, 23, 42, 0.6)", padding: "12px 16px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div style={{ fontSize: "12px", color: "var(--text-dim)", textTransform: "uppercase", letterSpacing: "0.5px" }}>Runtimes Protected</div>
              <div style={{ fontSize: "20px", fontWeight: 700, color: "#fbbf24", marginTop: "4px" }}>
                {(triageSummary.protected_cross_project_count + triageSummary.protected_vcs_count).toLocaleString()} files
              </div>
              <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "2px" }}>Unity / Git / Server guards</div>
            </div>

            <div style={{ background: "rgba(15, 23, 42, 0.6)", padding: "12px 16px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.06)" }}>
              <div style={{ fontSize: "12px", color: "var(--text-dim)", textTransform: "uppercase", letterSpacing: "0.5px" }}>Safety Net Window</div>
              <div style={{ fontSize: "20px", fontWeight: 700, color: "#a78bfa", marginTop: "4px" }}>
                30 Days
              </div>
              <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "2px" }}>100% reversible via Quarantine</div>
            </div>
          </div>
        </div>
      )}

      {totalGroups === 0 ? (
        <div className="empty-state">
          <h3>No duplicates found in this scope</h3>
          <p>Try switching to another scope filter or running a new scan.</p>
        </div>
      ) : (
        <>
          <div
            role="tablist"
            aria-label="Duplicate group scopes"
            style={{ display: "flex", gap: "8px", margin: "16px 0", flexWrap: "wrap", alignItems: "center" }}
          >
            <span style={{ fontSize: "13px", color: "var(--text-dim)", fontWeight: 600, marginRight: "4px" }}>Scope View:</span>
            <button
              role="tab"
              aria-selected={activeScope === "all"}
              className={`btn btn-small ${activeScope === "all" ? "btn-primary" : "btn-secondary"}`}
              onClick={() => { setActiveScope("all"); load(0, "all"); }}
            >
              All Groups ({triageSummary?.total_groups.toLocaleString() ?? totalGroups})
            </button>
            <button
              role="tab"
              aria-selected={activeScope === "personal"}
              className={`btn btn-small ${activeScope === "personal" ? "btn-primary" : "btn-secondary"}`}
              onClick={() => { setActiveScope("personal"); load(0, "personal"); }}
              title="Only show personal files: Downloads, Photos, Documents, Videos"
            >
              📁 Personal & Media ({triageSummary?.personal_media_groups.toLocaleString() ?? 0})
            </button>
            <button
              role="tab"
              aria-selected={activeScope === "heavy"}
              className={`btn btn-small ${activeScope === "heavy" ? "btn-primary" : "btn-secondary"}`}
              onClick={() => { setActiveScope("heavy"); load(0, "heavy"); }}
              title="High-impact duplicate groups larger than 50 MB"
            >
              📦 Heavy Files &gt;50MB ({triageSummary?.heavy_file_groups.toLocaleString() ?? 0})
            </button>
            <button
              role="tab"
              aria-selected={activeScope === "caches"}
              className={`btn btn-small ${activeScope === "caches" ? "btn-primary" : "btn-secondary"}`}
              onClick={() => { setActiveScope("caches"); load(0, "caches"); }}
              title="Unity PackageCache, node_modules, and git object duplicates"
            >
              ⚙️ Dev & Package Caches ({triageSummary?.dev_cache_groups.toLocaleString() ?? 0})
            </button>
            {activeScope === "caches" && (
              <button
                className="btn btn-ghost btn-small"
                style={{ marginLeft: "auto", color: "#fbbf24" }}
                onClick={handleWhitelistCaches}
                title="Add development cache folders to ignore list to reduce scan noise"
              >
                🛡️ Whitelist All Caches
              </button>
            )}
          </div>

          <div className="selection-bar" role="status">
            <span>
              <strong>{selIds.length}</strong> file(s) selected on this page ·{" "}
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

          {selIds.length > 0 && totalGroups > PAGE_SIZE && (
            <div
              style={{
                background: "rgba(59, 130, 246, 0.1)",
                border: "1px solid rgba(59, 130, 246, 0.25)",
                borderRadius: "8px",
                padding: "8px 14px",
                margin: "10px 0 16px 0",
                fontSize: "13px",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <span>
                {globalSelection
                  ? `All ${totalGroups.toLocaleString()} duplicate groups across all ${Math.ceil(totalGroups / PAGE_SIZE)} pages are selected for action.`
                  : `All ${groups.length} duplicate groups on this page are selected.`}
              </span>
              <button
                type="button"
                className="btn btn-ghost btn-small"
                style={{ color: "var(--accent)", fontWeight: 600 }}
                onClick={() => {
                  if (!globalSelection) {
                    setGlobalSelection(true);
                    setConfirmingAutoTriage(true);
                  } else {
                    setGlobalSelection(false);
                  }
                }}
              >
                {globalSelection
                  ? "Clear global selection"
                  : `👉 Select all ${totalGroups.toLocaleString()} duplicate groups across all ${Math.ceil(totalGroups / PAGE_SIZE)} pages`}
              </button>
            </div>
          )}


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
                              <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
                                <button
                                  type="button"
                                  className="btn-file-open-link"
                                  title="Click to open file in system default viewer"
                                  onClick={() => api.openFile(m.path)}
                                >
                                  <span className="file-open-icon">↗️</span>
                                  <span className="file-open-name">{m.file_name}</span>
                                </button>
                                <span className="dup-path-text">{m.path}</span>
                              </div>
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
                                  title="Open file in default system viewer"
                                  onClick={() => api.openFile(m.path)}
                                >
                                  ↗️ Open
                                </button>
                                <button
                                  type="button"
                                  className="btn-link-action"
                                  title="Reveal file in File Explorer"
                                  onClick={() => api.showInFolder(m.path)}
                                >
                                  📁 Reveal
                                </button>
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
                      onClick={() => selectAllCopiesExceptKeep(g)}
                      title="Keep only the canonical original copy and select all other copies for quarantine"
                    >
                      Keep One
                    </button>
                    <button
                      className="btn btn-secondary btn-small"
                      style={{ fontWeight: 600 }}
                      onClick={() => keepAllInGroup(g.id)}
                      title="Keep all copies in this group intact without deleting any"
                    >
                      Keep Both
                    </button>
                    <button
                      className="btn btn-secondary btn-small"
                      style={{ fontWeight: 600, color: "#38bdf8" }}
                      onClick={() => handleHardlinkMerge(g.id)}
                      disabled={busyMergeId === g.id}
                      title="Replace duplicates with native filesystem hardlinks. Reclaims disk space while keeping all file paths functioning!"
                    >
                      {busyMergeId === g.id ? "Merging…" : "🔗 Merge (Hardlink)"}
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

      {confirmingAutoTriage && triageSummary && (
        <div className="overlay-backdrop" role="dialog" aria-modal="true">
          <div className="modal" style={{ maxWidth: "560px" }}>
            <h2>Confirm Executive Auto-Triage</h2>
            <div className="modal-body">
              <p>
                You are approving automated cleanup across all <strong>{triageSummary.total_groups.toLocaleString()}</strong> duplicate groups:
              </p>
              <ul style={{ lineHeight: "1.8", margin: "16px 0", paddingLeft: "20px" }}>
                <li>
                  <strong>{triageSummary.recommended_keep_count.toLocaleString()}</strong> original files will be preserved in their permanent locations.
                </li>
                <li>
                  <strong>{triageSummary.recommended_quarantine_count.toLocaleString()}</strong> redundant copies will be safely moved to Quarantine.
                </li>
                <li>
                  <strong>{formatBytes(triageSummary.total_reclaimable_bytes)}</strong> of physical disk space will be reclaimed.
                </li>
                <li>
                  <strong>{triageSummary.protected_cross_project_count + triageSummary.protected_vcs_count}</strong> server runtimes and Git objects are automatically shielded and kept intact.
                </li>
              </ul>
              <div style={{ background: "rgba(16, 185, 129, 0.1)", border: "1px solid rgba(16, 185, 129, 0.25)", borderRadius: "8px", padding: "12px", fontSize: "13px", color: "var(--text-dim)" }}>
                🛡️ <strong>Option C 30-Day Safety Window:</strong> Nothing is permanently deleted today. All quarantined files can be restored with a single click at any time.
              </div>
            </div>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setConfirmingAutoTriage(false)} disabled={busy}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={handleApproveAutoTriage} disabled={busy}>
                {busy ? "Quarantining…" : "Approve & Move to Quarantine"}
              </button>
            </div>
          </div>
        </div>
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
