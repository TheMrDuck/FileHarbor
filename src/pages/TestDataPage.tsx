import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { api, formatBytes, formatDate } from "../api";
import type {
  GenerateTestDataRequest,
  TestGenerationProgress,
  TestManifestSummary,
  TestScenarioInfo,
  TrackedTestDataset,
  VerificationResult,
} from "../types";
import ConfirmDialog from "../components/ConfirmDialog";
import { IconAlert, IconShield } from "../components/NavIcons";

interface Props {
  onNavigateToScan: (folderPath: string) => void;
}

export default function TestDataPage({ onNavigateToScan }: Props) {
  const [tab, setTab] = useState<"generate" | "verify" | "clean">("generate");
  const [scenarios, setScenarios] = useState<TestScenarioInfo[]>([]);
  const [selectedScenario, setSelectedScenario] = useState<string>("phase1-baseline");
  const [outputDir, setOutputDir] = useState<string>("");
  const [allowDangerous, setAllowDangerous] = useState<boolean>(false);

  // Parameters
  const [filesCount, setFilesCount] = useState<number>(2500);
  const [seed, setSeed] = useState<number>(12345);
  const [duplicateGroups, setDuplicateGroups] = useState<number>(100);
  const [emptyFiles, setEmptyFiles] = useState<number>(50);
  const [unicodeFiles, setUnicodeFiles] = useState<number>(50);
  const [largeFiles, setLargeFiles] = useState<number>(5);
  const [nestedDepth, setNestedDepth] = useState<number>(5);
  const [sameSizeDiffContent, setSameSizeDiffContent] = useState<number>(50);
  const [showAdvanced, setShowAdvanced] = useState<boolean>(false);

  // Status & Progress
  const [isGenerating, setIsGenerating] = useState<boolean>(false);
  const [progress, setProgress] = useState<TestGenerationProgress | null>(null);
  const [summary, setSummary] = useState<TestManifestSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // Verification state
  const [verifyPath, setVerifyPath] = useState<string>("");
  const [isVerifying, setIsVerifying] = useState<boolean>(false);
  const [verifyResult, setVerifyResult] = useState<VerificationResult | null>(null);
  const [verifyError, setVerifyError] = useState<string | null>(null);

  // Clean state
  const [cleanPath, setCleanPath] = useState<string>("");
  const [isCleaning, setIsCleaning] = useState<boolean>(false);
  const [cleanConfirmOpen, setCleanConfirmOpen] = useState<boolean>(false);
  const [cleanResult, setCleanResult] = useState<string | null>(null);
  const [cleanError, setCleanError] = useState<string | null>(null);

  // Tracked test datasets
  const [trackedTestDatasets, setTrackedTestDatasets] = useState<TrackedTestDataset[]>([]);
  const [loadingTracked, setLoadingTracked] = useState<boolean>(false);
  const [datasetToDelete, setDatasetToDelete] = useState<TrackedTestDataset | null>(null);

  const refreshTrackedDatasets = () => {
    setLoadingTracked(true);
    api.listTrackedTestDatasets()
      .then((data) => setTrackedTestDatasets(data))
      .catch((err) => console.error("Failed to load tracked test datasets:", err))
      .finally(() => setLoadingTracked(false));
  };

  useEffect(() => {
    refreshTrackedDatasets();
  }, []);

  // Load scenarios on mount
  useEffect(() => {
    api.listTestScenarios()
      .then((data) => {
        setScenarios(data);
        if (data.length > 0 && !selectedScenario) {
          setSelectedScenario(data[0].name);
          setFilesCount(data[0].default_files);
        }
      })
      .catch((err) => setError(`Failed to list scenarios: ${err}`));
  }, []);

  // Update default path when scenario changes
  useEffect(() => {
    if (selectedScenario) {
      api.getDefaultTestDataDir(selectedScenario)
        .then((dir) => setOutputDir(dir))
        .catch(() => {});

      const found = scenarios.find((s) => s.name === selectedScenario);
      if (found) {
        setFilesCount(found.default_files);
      }
    }
  }, [selectedScenario, scenarios]);

  // Listen to testgen progress events
  useEffect(() => {
    const unlisten = listen<TestGenerationProgress>("testgen-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleBrowseOutput = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setOutputDir(selected);
    }
  };

  const handleBrowseVerify = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setVerifyPath(selected);
    }
  };

  const handleBrowseClean = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setCleanPath(selected);
    }
  };

  const handleGenerate = async () => {
    if (!outputDir) {
      setError("Please specify an output directory.");
      return;
    }
    setError(null);
    setSuccessMsg(null);
    setSummary(null);
    setIsGenerating(true);
    setProgress({
      files_created: 0,
      bytes_written: 0,
      directories_created: 0,
      duplicate_groups_created: 0,
    });

    const request: GenerateTestDataRequest = {
      scenario: selectedScenario,
      output_dir: outputDir,
      files: filesCount,
      seed,
      duplicate_groups: duplicateGroups,
      empty_files: emptyFiles,
      unicode_files: unicodeFiles,
      large_files: largeFiles,
      nested_depth: nestedDepth,
      same_size_diff_content: sameSizeDiffContent,
      allow_dangerous: allowDangerous,
    };

    try {
      const res = await api.generateTestData(request);
      setSummary(res);
      setSuccessMsg(`Dataset generated successfully in ${outputDir}!`);
      setVerifyPath(outputDir);
      setCleanPath(outputDir);
      refreshTrackedDatasets();
    } catch (err) {
      setError(String(err));
      setProgress(null);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleVerify = async (pathToVerify?: string) => {
    const target = pathToVerify || verifyPath;
    if (!target) {
      setVerifyError("Please specify a directory or manifest path to verify.");
      return;
    }
    setIsVerifying(true);
    setVerifyError(null);
    setVerifyResult(null);
    try {
      const result = await api.verifyTestData(target);
      setVerifyResult(result);
    } catch (err) {
      setVerifyError(String(err));
    } finally {
      setIsVerifying(false);
    }
  };

  const handleClean = async (targetPath?: string) => {
    const p = targetPath || cleanPath;
    if (!p) {
      setCleanError("Please specify a dataset directory to clean.");
      return;
    }
    setIsCleaning(true);
    setCleanError(null);
    setCleanResult(null);
    try {
      await api.cleanTestData(p);
      setCleanResult(`Successfully cleaned up ${p}`);
      if (verifyPath === p) setVerifyResult(null);
      if (outputDir === p) setSummary(null);
      refreshTrackedDatasets();
    } catch (err) {
      setCleanError(String(err));
    } finally {
      setIsCleaning(false);
      setCleanConfirmOpen(false);
      setDatasetToDelete(null);
    }
  };

  const currentScenarioInfo = scenarios.find((s) => s.name === selectedScenario);
  const percentComplete =
    isGenerating && progress && filesCount > 0
      ? Math.min(100, Math.round((progress.files_created / filesCount) * 100))
      : summary
      ? 100
      : 0;

  return (
    <section className="test-data-page">
      <div className="section-header">
        <div>
          <h2>Test Data Generator</h2>
          <p className="page-intro">
            Create deterministic, realistic file datasets with ground-truth manifests to test
            FileHarbor's duplicate detection, empty file handling, Unicode parsing, and backups.
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="tabs" style={{ display: "flex", gap: "8px", marginBottom: "20px" }}>
        <button
          className={tab === "generate" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => setTab("generate")}
        >
          Generate Dataset
        </button>
        <button
          className={tab === "verify" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => setTab("verify")}
        >
          Verify Dataset
        </button>
        <button
          className={tab === "clean" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => setTab("clean")}
        >
          Safe Cleanup
        </button>
      </div>

      {/* TAB 1: GENERATE DATASET */}
      {tab === "generate" && (
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          {error && <div className="card notice-banner error">{error}</div>}
          {successMsg && <div className="card notice-banner success">{successMsg}</div>}

          {/* Scenario Selection */}
          <div className="card">
            <h3>1. Choose Scenario</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: "12px", marginTop: "12px" }}>
              <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
                <label style={{ minWidth: "140px", fontWeight: 600 }}>Scenario:</label>
                <select
                  value={selectedScenario}
                  onChange={(e) => setSelectedScenario(e.target.value)}
                  disabled={isGenerating}
                  style={{
                    flex: 1,
                    padding: "8px 12px",
                    background: "var(--bg)",
                    color: "var(--text)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                    fontSize: "14px",
                  }}
                >
                  <optgroup label="Baseline & Duplicates">
                    <option value="phase1-baseline">phase1-baseline (Recommended for general testing)</option>
                    <option value="duplicates">duplicates (High duplicate ratio across directories)</option>
                    <option value="empty-files">empty-files (Zero-byte files, markers, .gitkeep)</option>
                    <option value="unicode">unicode (Arabic, Japanese, Chinese, Emoji names)</option>
                    <option value="large-files">large-files (Large files for SHA-256 stream test)</option>
                    <option value="regression">regression (Comprehensive Phase 1 regression)</option>
                  </optgroup>
                  <optgroup label="Performance & Scale">
                    <option value="performance-small">performance-small (1,000 files)</option>
                    <option value="performance-medium">performance-medium (10,000 files)</option>
                    <option value="performance-large">performance-large (100,000 files)</option>
                    <option value="performance-stress">performance-stress (1,000,000 files)</option>
                  </optgroup>
                  <optgroup label="Edge Cases & Stability">
                    <option value="recovered-hdd">recovered-hdd (file000001, recovered docs)</option>
                    <option value="unstable">unstable (Files that change during hash)</option>
                    <option value="missing-files">missing-files (Files deleted after manifest creation)</option>
                    <option value="symlinks">symlinks (Valid & broken symlinks)</option>
                    <option value="locked-files">locked-files (Windows locked files)</option>
                    <option value="permissions">permissions (Restricted file permissions)</option>
                  </optgroup>
                  <optgroup label="Backup Scenarios">
                    <option value="backup-baseline">backup-baseline (Baseline dataset for backup)</option>
                    <option value="backup-incremental">backup-incremental (Modified files for incremental backup)</option>
                    <option value="backup-changed-files">backup-changed-files (Only changed files)</option>
                    <option value="backup-deleted-files">backup-deleted-files (Deleted files test)</option>
                    <option value="backup-restore">backup-restore (Restore verification fixtures)</option>
                    <option value="backup-corruption">backup-corruption (Corrupted backup test)</option>
                    <option value="backup-interrupted">backup-interrupted (Interrupted backup simulation)</option>
                    <option value="backup-large">backup-large (10,000 files backup test)</option>
                    <option value="backup-duplicate-content">backup-duplicate-content (Backup duplicate deduplication)</option>
                  </optgroup>
                </select>
              </div>

              {currentScenarioInfo && (
                <div
                  style={{
                    background: "rgba(255,255,255,0.03)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                    padding: "12px 16px",
                  }}
                >
                  <p style={{ margin: "0 0 6px 0", color: "var(--text)" }}>
                    <strong>Description:</strong> {currentScenarioInfo.description}
                  </p>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)" }}>
                    Default file count: <strong>{currentScenarioInfo.default_files.toLocaleString()}</strong>
                  </span>
                </div>
              )}
            </div>
          </div>

          {/* Destination Directory */}
          <div className="card">
            <h3>2. Destination Directory</h3>
            <p style={{ fontSize: "13px", color: "var(--text-dim)", margin: "4px 0 12px 0" }}>
              Datasets are created inside a dedicated subfolder with a ground-truth <code>manifest.json</code>.
            </p>
            <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
              <input
                type="text"
                value={outputDir}
                onChange={(e) => setOutputDir(e.target.value)}
                disabled={isGenerating}
                placeholder="Target directory path..."
                aria-label="Target directory path for generated dataset"
                style={{
                  flex: 1,
                  padding: "8px 12px",
                  background: "var(--bg)",
                  color: "var(--text)",
                  border: "1px solid var(--border)",
                  borderRadius: "var(--radius)",
                  fontSize: "14px",
                }}
              />
              <button
                className="btn btn-secondary"
                onClick={handleBrowseOutput}
                disabled={isGenerating}
              >
                Browse...
              </button>
            </div>
            <div style={{ marginTop: "10px" }}>
              <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "13px", color: "var(--text-dim)", cursor: "pointer" }}>
                <input
                  type="checkbox"
                  checked={allowDangerous}
                  onChange={(e) => setAllowDangerous(e.target.checked)}
                  disabled={isGenerating}
                />
                Allow custom or root directories (overrides safety boundary check)
              </label>
            </div>
          </div>

          {/* Configuration Parameters */}
          <div className="card">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <h3>3. Configuration</h3>
              <button
                className="btn btn-secondary"
                style={{ fontSize: "12px", padding: "4px 10px" }}
                onClick={() => setShowAdvanced(!showAdvanced)}
              >
                {showAdvanced ? "Hide Advanced Options" : "Show Advanced Options"}
              </button>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "16px", marginTop: "16px" }}>
              <div>
                <label style={{ display: "block", fontSize: "13px", marginBottom: "6px" }}>
                  Total Files: <strong>{filesCount.toLocaleString()}</strong>
                </label>
                <input
                  type="number"
                  min={1}
                  max={1000000}
                  value={filesCount}
                  onChange={(e) => setFilesCount(Math.max(1, parseInt(e.target.value) || 0))}
                  disabled={isGenerating}
                  aria-label="Total files to generate"
                  style={{
                    width: "100%",
                    padding: "8px 12px",
                    background: "var(--bg)",
                    color: "var(--text)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                  }}
                />
              </div>

              <div>
                <label style={{ display: "block", fontSize: "13px", marginBottom: "6px" }}>
                  Random Seed (for deterministic generation):
                </label>
                <input
                  type="number"
                  value={seed}
                  onChange={(e) => setSeed(parseInt(e.target.value) || 0)}
                  disabled={isGenerating}
                  aria-label="Random seed for deterministic generation"
                  style={{
                    width: "100%",
                    padding: "8px 12px",
                    background: "var(--bg)",
                    color: "var(--text)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                  }}
                />
              </div>
            </div>

            {showAdvanced && (
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(3, 1fr)",
                  gap: "16px",
                  marginTop: "16px",
                  paddingTop: "16px",
                  borderTop: "1px solid var(--border)",
                }}
              >
                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Duplicate Groups:
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={duplicateGroups}
                    onChange={(e) => setDuplicateGroups(parseInt(e.target.value) || 0)}
                    disabled={isGenerating}
                    aria-label="Duplicate groups count"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>

                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Empty Files:
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={emptyFiles}
                    onChange={(e) => setEmptyFiles(parseInt(e.target.value) || 0)}
                    disabled={isGenerating}
                    aria-label="Empty files count"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>

                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Unicode Files:
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={unicodeFiles}
                    onChange={(e) => setUnicodeFiles(parseInt(e.target.value) || 0)}
                    disabled={isGenerating}
                    aria-label="Unicode files count"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>

                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Large Files:
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={largeFiles}
                    onChange={(e) => setLargeFiles(parseInt(e.target.value) || 0)}
                    disabled={isGenerating}
                    aria-label="Large files count"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>

                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Max Nested Depth:
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={20}
                    value={nestedDepth}
                    onChange={(e) => setNestedDepth(parseInt(e.target.value) || 1)}
                    disabled={isGenerating}
                    aria-label="Max nested directory depth"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>

                <div>
                  <label style={{ display: "block", fontSize: "12px", marginBottom: "4px", color: "var(--text-dim)" }}>
                    Same-Size Diff Content:
                  </label>
                  <input
                    type="number"
                    min={0}
                    value={sameSizeDiffContent}
                    onChange={(e) => setSameSizeDiffContent(parseInt(e.target.value) || 0)}
                    disabled={isGenerating}
                    aria-label="Same-size different-content files count"
                    style={{
                      width: "100%",
                      padding: "6px 10px",
                      background: "var(--bg)",
                      color: "var(--text)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius)",
                    }}
                  />
                </div>
              </div>
            )}

            <div style={{ marginTop: "20px" }}>
              <button
                className="btn btn-primary"
                onClick={handleGenerate}
                disabled={isGenerating}
                style={{ padding: "10px 24px", fontSize: "15px" }}
              >
                {isGenerating ? "Generating Dataset..." : "Generate Test Dataset"}
              </button>
            </div>

            {error && (
              <div
                style={{
                  marginTop: "16px",
                  padding: "12px 16px",
                  borderRadius: "var(--radius)",
                  background: "rgba(229, 83, 75, 0.15)",
                  border: "1px solid var(--danger)",
                  color: "var(--danger)",
                  fontSize: "13px",
                  lineHeight: "1.5",
                }}
              >
                <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                  <IconAlert size={14} />
                  <span>Generation Error:</span>
                </strong> {error}
              </div>
            )}
          </div>

          {/* Progress Card (during generation or after success) */}
          {(isGenerating || (progress && summary)) && (
            <div className="card progress-card" style={{ textAlign: "left" }}>
              <div className="progress-header">
                <h3 style={{ margin: 0 }}>
                  {isGenerating ? "Generating Dataset..." : "Generation Complete"}
                </h3>
                <span style={{ fontWeight: 600, color: "var(--accent)" }}>{percentComplete}%</span>
              </div>

              <div className="progress-bar-container" style={{ margin: "16px 0 12px 0" }}>
                <div
                  className="progress-bar"
                  style={{ width: `${percentComplete}%` }}
                />
              </div>

              <div className="progress-stats" style={{ gridTemplateColumns: "repeat(4, 1fr)", marginTop: "16px" }}>
                <div className="stat">
                  <span className="stat-label" style={{ fontSize: "12px", color: "var(--text-dim)" }}>Files Created</span>
                  <span className="stat-value" style={{ fontSize: "18px", fontWeight: 600 }}>
                    {progress?.files_created.toLocaleString() ?? 0} / {filesCount.toLocaleString()}
                  </span>
                </div>
                <div className="stat">
                  <span className="stat-label" style={{ fontSize: "12px", color: "var(--text-dim)" }}>Bytes Written</span>
                  <span className="stat-value" style={{ fontSize: "18px", fontWeight: 600 }}>
                    {formatBytes(progress?.bytes_written ?? 0)}
                  </span>
                </div>
                <div className="stat">
                  <span className="stat-label" style={{ fontSize: "12px", color: "var(--text-dim)" }}>Duplicate Groups</span>
                  <span className="stat-value" style={{ fontSize: "18px", fontWeight: 600 }}>
                    {progress?.duplicate_groups_created.toLocaleString() ?? 0}
                  </span>
                </div>
                <div className="stat">
                  <span className="stat-label" style={{ fontSize: "12px", color: "var(--text-dim)" }}>Directories</span>
                  <span className="stat-value" style={{ fontSize: "18px", fontWeight: 600 }}>
                    {progress?.directories_created.toLocaleString() ?? 0}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* Completion & Next Actions Card */}
          {summary && (
            <div
              className="card"
              style={{
                border: "1px solid var(--success)",
                background: "rgba(79, 191, 122, 0.05)",
              }}
            >
              <h3 style={{ color: "var(--success)", display: "flex", alignItems: "center", gap: "8px" }}>
                <span>✓</span> Dataset Ready for Testing!
              </h3>
              <p style={{ margin: "8px 0 16px 0", color: "var(--text)" }}>
                Synthetic test dataset generated into <code>{outputDir}</code>.
              </p>

              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(5, 1fr)",
                  gap: "12px",
                  padding: "12px",
                  background: "var(--bg)",
                  borderRadius: "var(--radius)",
                  border: "1px solid var(--border)",
                  marginBottom: "20px",
                }}
              >
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Total Files</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{summary.files}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Duplicate Groups</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{summary.duplicate_groups}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Duplicate Copies</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{summary.duplicate_files}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Empty Files</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{summary.empty_files}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Directories</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{summary.directories}</span>
                </div>
              </div>

              {/* Action Buttons */}
              <div style={{ display: "flex", gap: "12px", flexWrap: "wrap" }}>
                <button
                  className="btn btn-primary"
                  onClick={() => onNavigateToScan(outputDir)}
                  style={{ padding: "10px 20px", fontSize: "14px", fontWeight: 600 }}
                >
                  Scan this Dataset in FileHarbor
                </button>
                <button
                  className="btn btn-secondary"
                  onClick={() => {
                    setTab("verify");
                    setVerifyPath(outputDir);
                    handleVerify(outputDir);
                  }}
                  style={{ padding: "10px 16px" }}
                >
                  Verify against Manifest
                </button>
                <button
                  className="btn btn-secondary"
                  onClick={() => {
                    setTab("clean");
                    setCleanPath(outputDir);
                  }}
                  style={{ padding: "10px 16px" }}
                >
                  Safe Cleanup
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* TAB 2: VERIFY DATASET */}
      {tab === "verify" && (
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          {verifyError && <div className="card notice-banner error">{verifyError}</div>}

          <div className="card">
            <h3>Verify Dataset Integrity</h3>
            <p style={{ fontSize: "13px", color: "var(--text-dim)", margin: "4px 0 16px 0" }}>
              Verify an existing dataset against its ground-truth <code>manifest.json</code> to ensure files,
              hashes, and sizes haven't been corrupted or modified.
            </p>

            <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
              <input
                type="text"
                value={verifyPath}
                onChange={(e) => setVerifyPath(e.target.value)}
                placeholder="Dataset directory or manifest.json path..."
                aria-label="Dataset directory or manifest path to verify"
                disabled={isVerifying}
                style={{
                  flex: 1,
                  padding: "8px 12px",
                  background: "var(--bg)",
                  color: "var(--text)",
                  border: "1px solid var(--border)",
                  borderRadius: "var(--radius)",
                }}
              />
              <button
                className="btn btn-secondary"
                onClick={handleBrowseVerify}
                disabled={isVerifying}
              >
                Browse...
              </button>
              <button
                className="btn btn-primary"
                onClick={() => handleVerify()}
                disabled={isVerifying || !verifyPath}
              >
                {isVerifying ? "Verifying..." : "Run Verification"}
              </button>
            </div>
          </div>

          {verifyResult && (
            <div className="card">
              <div style={{ display: "flex", alignItems: "center", gap: "12px", marginBottom: "16px" }}>
                <h3>Verification Report</h3>
                {verifyResult.missing_files.length === 0 &&
                verifyResult.unexpected_files.length === 0 &&
                verifyResult.size_mismatches.length === 0 &&
                verifyResult.hash_mismatches.length === 0 ? (
                  <span
                    style={{
                      background: "rgba(79, 191, 122, 0.15)",
                      color: "var(--success)",
                      padding: "4px 10px",
                      borderRadius: "var(--radius)",
                      fontWeight: 600,
                      fontSize: "13px",
                    }}
                  >
                    ✓ All Checks Passed
                  </span>
                ) : (
                  <span
                    style={{
                      background: "rgba(229, 83, 75, 0.15)",
                      color: "var(--danger)",
                      padding: "4px 10px",
                      borderRadius: "var(--radius)",
                      fontWeight: 600,
                      fontSize: "13px",
                      display: "inline-flex",
                      alignItems: "center",
                      gap: "6px",
                    }}
                  >
                    <IconAlert size={14} />
                    <span>Discrepancies Found</span>
                  </span>
                )}
              </div>

              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(4, 1fr)",
                  gap: "12px",
                  padding: "12px",
                  background: "var(--bg)",
                  borderRadius: "var(--radius)",
                  border: "1px solid var(--border)",
                  marginBottom: "16px",
                }}
              >
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Expected Files</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{verifyResult.expected_files}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Found Files</span>
                  <span style={{ fontSize: "16px", fontWeight: 600 }}>{verifyResult.found_files}</span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Missing Files</span>
                  <span style={{ fontSize: "16px", fontWeight: 600, color: verifyResult.missing_files.length > 0 ? "var(--danger)" : "inherit" }}>
                    {verifyResult.missing_files.length}
                  </span>
                </div>
                <div>
                  <span style={{ fontSize: "12px", color: "var(--text-dim)", display: "block" }}>Hash Mismatches</span>
                  <span style={{ fontSize: "16px", fontWeight: 600, color: verifyResult.hash_mismatches.length > 0 ? "var(--danger)" : "inherit" }}>
                    {verifyResult.hash_mismatches.length}
                  </span>
                </div>
              </div>

              {verifyResult.missing_files.length > 0 && (
                <div style={{ marginTop: "16px" }}>
                  <h4 style={{ color: "var(--danger)", margin: "0 0 8px 0" }}>Missing Files:</h4>
                  <ul style={{ maxHeight: "150px", overflowY: "auto", margin: 0, paddingLeft: "20px" }}>
                    {verifyResult.missing_files.map((file, i) => (
                      <li key={i} style={{ fontSize: "13px", color: "var(--text-dim)" }}>{file}</li>
                    ))}
                  </ul>
                </div>
              )}

              {verifyResult.size_mismatches.length > 0 && (
                <div style={{ marginTop: "16px" }}>
                  <h4 style={{ color: "var(--danger)", margin: "0 0 8px 0" }}>Size Mismatches:</h4>
                  <ul style={{ maxHeight: "150px", overflowY: "auto", margin: 0, paddingLeft: "20px" }}>
                    {verifyResult.size_mismatches.map((m, i) => (
                      <li key={i} style={{ fontSize: "13px", color: "var(--text-dim)" }}>
                        {m.path} (expected: {m.expected} bytes, actual: {m.actual} bytes)
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {verifyResult.hash_mismatches.length > 0 && (
                <div style={{ marginTop: "16px" }}>
                  <h4 style={{ color: "var(--danger)", margin: "0 0 8px 0" }}>Hash Mismatches:</h4>
                  <ul style={{ maxHeight: "150px", overflowY: "auto", margin: 0, paddingLeft: "20px" }}>
                    {verifyResult.hash_mismatches.map((m, i) => (
                      <li key={i} style={{ fontSize: "13px", color: "var(--text-dim)" }}>
                        {m.path}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* TAB 3: SAFE CLEANUP */}
      {tab === "clean" && (
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          {cleanError && <div className="card notice-banner error">{cleanError}</div>}
          {cleanResult && <div className="card notice-banner success">{cleanResult}</div>}

          <div className="card">
            <h3>Safe Dataset Cleanup</h3>
            <p style={{ fontSize: "13px", color: "var(--text-dim)", margin: "4px 0 16px 0" }}>
              Safely delete test data directories when testing is complete.
            </p>

            <div
              style={{
                background: "rgba(217, 165, 63, 0.1)",
                border: "1px solid var(--warning)",
                borderRadius: "var(--radius)",
                padding: "12px 16px",
                marginBottom: "16px",
              }}
            >
              <strong style={{ color: "var(--warning)", display: "inline-flex", alignItems: "center", gap: "6px" }}>
                <IconShield size={15} />
                <span>Safety Protection:</span>
              </strong>
              <p style={{ margin: "4px 0 0 0", fontSize: "13px", color: "var(--text)" }}>
                Cleanup verifies that the target folder contains a valid FileHarbor <code>manifest.json</code> before deleting anything.
                It will strictly refuse to touch real user directories, documents, or systems folders.
              </p>
            </div>

            <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
              <input
                type="text"
                value={cleanPath}
                onChange={(e) => setCleanPath(e.target.value)}
                placeholder="Path to generated test dataset directory..."
                aria-label="Test dataset directory path to clean up"
                disabled={isCleaning}
                style={{
                  flex: 1,
                  padding: "8px 12px",
                  background: "var(--bg)",
                  color: "var(--text)",
                  border: "1px solid var(--border)",
                  borderRadius: "var(--radius)",
                }}
              />
              <button
                className="btn btn-secondary"
                onClick={handleBrowseClean}
                disabled={isCleaning}
              >
                Browse...
              </button>
              <button
                className="btn btn-danger"
                onClick={() => setCleanConfirmOpen(true)}
                disabled={isCleaning || !cleanPath}
              >
                {isCleaning ? "Cleaning..." : "Clean Dataset"}
              </button>
            </div>
          </div>

          {cleanConfirmOpen && (
            <ConfirmDialog
              title="Clean Test Dataset"
              body={`Are you sure you want to permanently delete the test dataset at: ${cleanPath}? Only datasets with a valid manifest will be cleaned.`}
              confirmLabel="Delete Test Dataset"
              danger
              onConfirm={() => handleClean()}
              onCancel={() => setCleanConfirmOpen(false)}
            />
          )}

          {/* Tracked Test Datasets Table */}
          <div className="card">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "12px" }}>
              <div>
                <h3 style={{ margin: 0 }}>Tracked Test Datasets</h3>
                <p style={{ fontSize: "13px", color: "var(--text-dim)", margin: "4px 0 0 0" }}>
                  All datasets generated by FileHarbor are recorded here for effortless management and safe removal.
                </p>
              </div>
              <button
                className="btn btn-secondary"
                onClick={refreshTrackedDatasets}
                disabled={loadingTracked}
                style={{ fontSize: "12px", padding: "4px 10px" }}
              >
                {loadingTracked ? "Refreshing..." : "Refresh"}
              </button>
            </div>

            {trackedTestDatasets.length === 0 ? (
              <div style={{ padding: "24px 0", textAlign: "center", color: "var(--text-dim)", fontSize: "14px" }}>
                No tracked test datasets found. Datasets you generate will automatically appear here.
              </div>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "13px", textAlign: "left" }}>
                  <thead>
                    <tr style={{ borderBottom: "1px solid var(--border)", color: "var(--text-dim)" }}>
                      <th style={{ padding: "8px 10px" }}>Scenario</th>
                      <th style={{ padding: "8px 10px" }}>Location</th>
                      <th style={{ padding: "8px 10px" }}>Files</th>
                      <th style={{ padding: "8px 10px" }}>Total Size</th>
                      <th style={{ padding: "8px 10px" }}>Created</th>
                      <th style={{ padding: "8px 10px", textAlign: "right" }}>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {trackedTestDatasets.map((ds) => (
                      <tr key={ds.id} style={{ borderBottom: "1px solid var(--border)" }}>
                        <td style={{ padding: "10px", fontWeight: 600 }}>{ds.scenario}</td>
                        <td
                          style={{
                            padding: "10px",
                            fontFamily: "monospace",
                            fontSize: "12px",
                            maxWidth: "260px",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                          title={ds.path}
                        >
                          {ds.path}
                        </td>
                        <td style={{ padding: "10px" }}>{ds.total_files.toLocaleString()}</td>
                        <td style={{ padding: "10px" }}>{formatBytes(ds.total_bytes)}</td>
                        <td style={{ padding: "10px", color: "var(--text-dim)" }}>{formatDate(ds.created_at_ms)}</td>
                        <td style={{ padding: "10px", textAlign: "right" }}>
                          <button
                            className="btn btn-danger"
                            style={{ fontSize: "12px", padding: "4px 10px" }}
                            disabled={isCleaning}
                            onClick={() => setDatasetToDelete(ds)}
                          >
                            Delete
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {datasetToDelete && (
            <ConfirmDialog
              title="Delete Tracked Test Dataset"
              body={`Are you sure you want to completely delete the generated test dataset (${datasetToDelete.scenario}, ${datasetToDelete.total_files.toLocaleString()} files) located at:\n${datasetToDelete.path}?`}
              confirmLabel="Delete Dataset"
              danger
              onConfirm={() => handleClean(datasetToDelete.path)}
              onCancel={() => setDatasetToDelete(null)}
            />
          )}
        </div>
      )}
    </section>
  );
}
