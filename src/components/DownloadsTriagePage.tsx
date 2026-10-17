import React, { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { downloadDir, desktopDir, documentDir } from "@tauri-apps/api/path";
import { api, formatBytes } from "../api";
import type { TriagePlan } from "../types";

export const FolderTriagePage: React.FC = () => {
  const [folderPath, setFolderPath] = useState<string>("");
  const [organizeByCategory, setOrganizeByCategory] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(false);
  const [plan, setPlan] = useState<TriagePlan | null>(null);
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [executing, setExecuting] = useState<boolean>(false);

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setFolderPath(selected);
      }
    } catch {
      // ignore
    }
  };

  const setPresetPath = async (getter: () => Promise<string>) => {
    try {
      const path = await getter();
      if (path) {
        setFolderPath(path);
      }
    } catch {
      // ignore
    }
  };

  const handleScan = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanTriagePlan({
        folder_path: folderPath.trim(),
        organize_by_category: organizeByCategory,
      });
      setPlan(res);
      // By default select all items for organization
      setSelectedItems(new Set(res.items.map((i) => i.file_path)));
    } catch (err) {
      setStatusMessage(`Scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const toggleSelect = (path: string) => {
    const next = new Set(selectedItems);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    setSelectedItems(next);
  };

  const handleSelectAll = (select: boolean) => {
    if (!plan) return;
    if (select) {
      setSelectedItems(new Set(plan.items.map((i) => i.file_path)));
    } else {
      setSelectedItems(new Set());
    }
  };

  const handleExecuteMoves = async () => {
    if (!plan) return;
    const itemsToMove: [string, string][] = plan.items
      .filter((i) => selectedItems.has(i.file_path))
      .map((i) => [i.file_path, i.suggested_destination]);

    if (itemsToMove.length === 0) {
      setStatusMessage("No files selected for organization.");
      return;
    }

    setExecuting(true);
    setStatusMessage(null);
    try {
      const res = await api.executeTriageMoves({ items_to_move: itemsToMove });
      setStatusMessage(
        `Successfully organized ${res.moved_count} file(s) into category subfolders!${
          res.failed_items.length > 0
            ? ` (${res.failed_items.length} failed)`
            : ""
        }`
      );
      // Refresh scan
      handleScan();
    } catch (err) {
      setStatusMessage(`Organization execution failed: ${String(err)}`);
    } finally {
      setExecuting(false);
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      <div className="page-header">
        <div>
          <h2>Folder Triage &amp; Clutter Organizer</h2>
          <p className="page-intro">
            Triage, clean up, and organize any messy directory — Downloads, Desktop, external drives, or unorganized work folders — neatly sorting loose clutter into categorized subfolders.
          </p>
        </div>
      </div>

      <div className="card">
        <label style={{ display: "block", fontSize: "13px", fontWeight: 600, marginBottom: "8px" }}>
          Target Directory to Triage:
        </label>
        <div style={{ display: "flex", gap: "12px", flexWrap: "wrap", alignItems: "center" }}>
          <input
            type="text"
            style={{ flex: 1, minWidth: "260px" }}
            placeholder="Select or enter any messy folder (e.g. Downloads, Desktop, USB drive, project dump)..."
            value={folderPath}
            onChange={(e) => setFolderPath(e.target.value)}
            aria-label="Triage target folder path"
          />
          <button
            type="button"
            onClick={handleBrowseFolder}
            className="btn btn-secondary"
          >
            Browse Any Folder…
          </button>
          <button
            onClick={handleScan}
            disabled={loading || !folderPath.trim()}
            className="btn btn-primary"
          >
            {loading ? "Analyzing Folder..." : "Analyze Folder"}
          </button>
        </div>

        {/* Quick Folder Presets */}
        <div style={{ marginTop: "12px", display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap", fontSize: "12px" }}>
          <span style={{ color: "var(--text-dim)" }}>Quick Presets:</span>
          <button
            type="button"
            className="btn btn-ghost btn-small"
            style={{ padding: "3px 8px", fontSize: "12px" }}
            onClick={() => setPresetPath(downloadDir)}
            title="Set target to your Downloads folder"
          >
            📥 Downloads
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-small"
            style={{ padding: "3px 8px", fontSize: "12px" }}
            onClick={() => setPresetPath(desktopDir)}
            title="Set target to your Desktop folder"
          >
            🖥️ Desktop
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-small"
            style={{ padding: "3px 8px", fontSize: "12px" }}
            onClick={() => setPresetPath(documentDir)}
            title="Set target to your Documents folder"
          >
            📄 Documents
          </button>
        </div>

        <div style={{ marginTop: "14px", display: "flex", alignItems: "center", gap: "8px" }}>
          <label style={{ display: "inline-flex", alignItems: "center", gap: "8px", cursor: "pointer", fontSize: "13px" }}>
            <input
              type="checkbox"
              checked={organizeByCategory}
              onChange={(e) => setOrganizeByCategory(e.target.checked)}
              aria-label="Sort into subfolders by category"
            />
            Sort into Subfolders (Documents, Images, Videos, Installers, Archives, Code, etc.)
          </label>
        </div>
      </div>

      {statusMessage && (
        <div className="card" style={{ padding: "12px 16px", borderColor: "var(--accent)" }}>
          {statusMessage}
        </div>
      )}

      {!plan && !loading && (
        <div className="card" style={{ padding: "24px", color: "var(--text-dim)", fontSize: "13px", lineHeight: "1.6" }}>
          <h3 style={{ margin: "0 0 8px 0", color: "var(--text)", fontSize: "15px" }}>
            💡 Universal Clutter Triage
          </h3>
          <p style={{ margin: "0 0 8px 0" }}>
            Whether it's your browser Downloads, cluttered Desktop, an old USB drive, or a disorganized archive, FileHarbor identifies file types through extensions and magic-bytes, builds a proposed reorganization tree, and safely moves files with collision protection.
          </p>
          <p style={{ margin: 0 }}>
            Select a folder above and click <strong>Analyze Folder</strong> to preview the organized structure before making any changes.
          </p>
        </div>
      )}

      {plan && (
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          {/* Summary Metric Cards */}
          <div className="stat-grid">
            <div className="stat-card">
              <div className="stat-label">Total Files Found</div>
              <div className="stat-value">{plan.items.length}</div>
              <div className="stat-sub">{formatBytes(plan.total_size_bytes)}</div>
            </div>

            <div className="stat-card">
              <div className="stat-label" style={{ color: "var(--warning)" }}>Stale Installers</div>
              <div className="stat-value" style={{ color: "var(--warning)" }}>{plan.stale_installers_count}</div>
              <div className="stat-sub">Setup packages &gt;30d old</div>
            </div>

            <div className="stat-card">
              <div className="stat-label" style={{ color: "var(--danger)" }}>Incomplete / Temp</div>
              <div className="stat-value" style={{ color: "var(--danger)" }}>{plan.incomplete_downloads_count}</div>
              <div className="stat-sub">.crdownload / .part files</div>
            </div>

            <div className="stat-card">
              <div className="stat-label">Identified Categories</div>
              <div className="stat-value">{Object.keys(plan.category_counts).length}</div>
              <div className="stat-sub">Structured destinations</div>
            </div>
          </div>

          {/* Action Header */}
          <div className="card" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "12px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
              <span style={{ fontSize: "14px" }}>
                <strong style={{ color: "var(--accent)" }}>{selectedItems.size}</strong> of {plan.items.length} files selected
              </span>
              <button
                onClick={() => handleSelectAll(selectedItems.size !== plan.items.length)}
                className="btn btn-ghost btn-small"
              >
                {selectedItems.size === plan.items.length ? "Deselect All" : "Select All"}
              </button>
            </div>

            <button
              onClick={handleExecuteMoves}
              disabled={executing || selectedItems.size === 0}
              className="btn btn-primary"
            >
              {executing
                ? "Organizing Files..."
                : `Apply Organization (${selectedItems.size})`}
            </button>
          </div>

          {/* File Items Table */}
          <div className="card" style={{ padding: 0, overflow: "hidden" }}>
            <div className="table-container" style={{ maxHeight: "500px", overflowY: "auto" }}>
              <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "13px" }}>
                <thead>
                  <tr style={{ borderBottom: "1px solid var(--border)", background: "var(--bg-panel)" }}>
                    <th style={{ padding: "10px", width: "40px", textAlign: "center" }}>
                      <input
                        type="checkbox"
                        checked={selectedItems.size === plan.items.length && plan.items.length > 0}
                        onChange={(e) => handleSelectAll(e.target.checked)}
                        aria-label="Select all triage files"
                      />
                    </th>
                    <th style={{ padding: "10px", textAlign: "left" }}>File Name</th>
                    <th style={{ padding: "10px", textAlign: "left" }}>Category</th>
                    <th style={{ padding: "10px", textAlign: "left" }}>Size</th>
                    <th style={{ padding: "10px", textAlign: "left" }}>Flags</th>
                    <th style={{ padding: "10px", textAlign: "left" }}>Target Subfolder</th>
                    <th style={{ padding: "10px", textAlign: "right" }}>Inspect</th>
                  </tr>
                </thead>
                <tbody>
                  {plan.items.map((item) => {
                    const isSelected = selectedItems.has(item.file_path);
                    return (
                      <tr
                        key={item.file_path}
                        style={{
                          borderBottom: "1px solid var(--border)",
                          background: isSelected ? "rgba(79, 140, 255, 0.08)" : "transparent",
                          cursor: "pointer",
                        }}
                        onClick={() => toggleSelect(item.file_path)}
                      >
                        <td style={{ padding: "10px", textAlign: "center" }} onClick={(e) => e.stopPropagation()}>
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => toggleSelect(item.file_path)}
                            aria-label={`Select file ${item.file_name}`}
                          />
                        </td>
                        <td style={{ padding: "10px", fontWeight: 500 }}>
                          {item.file_name}
                        </td>
                        <td style={{ padding: "10px" }}>
                          <span className="badge" style={{ background: "rgba(255, 255, 255, 0.08)" }}>
                            {item.category}
                          </span>
                        </td>
                        <td style={{ padding: "10px", color: "var(--text-dim)" }}>
                          {formatBytes(item.file_size)}
                        </td>
                        <td style={{ padding: "10px" }}>
                          {item.is_stale_installer && (
                            <span className="badge" style={{ borderColor: "var(--warning)", color: "var(--warning)", marginRight: "6px" }}>
                              Stale Installer
                            </span>
                          )}
                          {item.is_incomplete_download && (
                            <span className="badge" style={{ borderColor: "var(--danger)", color: "var(--danger)" }}>
                              Incomplete
                            </span>
                          )}
                        </td>
                        <td className="mono truncated" style={{ padding: "10px", color: "var(--text-dim)" }} title={item.suggested_destination}>
                          {item.suggested_destination.split(/[\\/]/).slice(-2).join("/")}
                        </td>
                        <td style={{ padding: "10px", textAlign: "right" }} onClick={(e) => e.stopPropagation()}>
                          <div style={{ display: "inline-flex", gap: "6px" }}>
                            <button
                              type="button"
                              className="btn btn-ghost btn-small"
                              style={{ padding: "2px 6px", fontSize: "11px" }}
                              onClick={() => api.openFile(item.file_path).catch(() => {})}
                              title="Open file in default system application"
                            >
                              ↗️ Open
                            </button>
                            <button
                              type="button"
                              className="btn btn-ghost btn-small"
                              style={{ padding: "2px 6px", fontSize: "11px" }}
                              onClick={() => api.showInFolder(item.file_path).catch(() => {})}
                              title="Reveal file in File Explorer"
                            >
                              📁 Reveal
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export const DownloadsTriagePage = FolderTriagePage;
