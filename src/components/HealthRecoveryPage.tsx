import React, { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, formatBytes } from "../api";
import { IconAlert, IconCheck, IconTrash } from "./NavIcons";
import type {
  ExtensionRectificationProposal,
  NearDuplicateCluster,
  ReconstructionPlan,
  ReconstructionResult,
  RecoveryPairComparison,
  DocumentLineageCluster,
  CarvedFileProposal,
} from "../types";

interface HealthRecoveryPageProps {
  initialFolderPath?: string;
}

export const HealthRecoveryPage: React.FC<HealthRecoveryPageProps> = ({ initialFolderPath }) => {
  const [activeTab, setActiveTab] = useState<"collisions" | "rectifier" | "near-dupes" | "reconstruction" | "doc-lineage" | "carving">("collisions");
  const [folderPath, setFolderPath] = useState<string>(initialFolderPath || "");
  const [loading, setLoading] = useState<boolean>(false);
  const [loadingAll, setLoadingAll] = useState<boolean>(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  React.useEffect(() => {
    if (initialFolderPath) {
      setFolderPath(initialFolderPath);
    }
  }, [initialFolderPath]);

  // Quarantine in-progress flag
  const [quarantining, setQuarantining] = useState<boolean>(false);

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

  const handleBrowseCandidateA = async () => {
    try {
      const selected = await open({ directory: false, multiple: false });
      if (typeof selected === "string") {
        setCandidateAPath(selected);
      }
    } catch {
      // ignore
    }
  };

  const handleBrowseCandidateB = async () => {
    try {
      const selected = await open({ directory: false, multiple: false });
      if (typeof selected === "string") {
        setCandidateBPath(selected);
      }
    } catch {
      // ignore
    }
  };

  // Tab 1: Recovery Collisions
  const [pairs, setPairs] = useState<RecoveryPairComparison[]>([]);

  // Tab 2: Extension Rectifier
  const [rectifications, setRectifications] = useState<ExtensionRectificationProposal[]>([]);
  const [selectedRectifications, setSelectedRectifications] = useState<Set<string>>(new Set());
  const [rectifying, setRectifying] = useState<boolean>(false);

  // Tab 3: Perceptual Near-Duplicates
  const [clusters, setClusters] = useState<NearDuplicateCluster[]>([]);
  const [hammingDistanceThreshold, setHammingDistanceThreshold] = useState<number>(6);

  // Tab 4: Multi-Copy Reconstruction
  const [candidateAPath, setCandidateAPath] = useState<string>("");
  const [candidateBPath, setCandidateBPath] = useState<string>("");
  const [reconstructionPlan, setReconstructionPlan] = useState<ReconstructionPlan | null>(null);
  const [reconstructionResult, setReconstructionResult] = useState<ReconstructionResult | null>(null);
  const [isReconstructing, setIsReconstructing] = useState<boolean>(false);

  // Tab 5: Document Lineage & Revisions
  const [docClusters, setDocClusters] = useState<DocumentLineageCluster[]>([]);
  const [docDistanceThreshold, setDocDistanceThreshold] = useState<number>(12);

  // Tab 6: Deep Carving & Metadata Renamer
  const [carvingProposals, setCarvingProposals] = useState<CarvedFileProposal[]>([]);
  const [selectedCarvings, setSelectedCarvings] = useState<Set<string>>(new Set());
  const [carvingExecuting, setCarvingExecuting] = useState<boolean>(false);

  const handleRunAllAudits = async () => {
    if (!folderPath.trim()) return;
    setLoadingAll(true);
    setStatusMessage(null);
    try {
      const path = folderPath.trim();
      const [collRes, rectRes, nearRes, docRes, carvRes] = await Promise.allSettled([
        api.scanAndCompareRecoveryPairs(path),
        api.scanExtensionRectifications(path),
        api.scanPerceptualDuplicates(path, hammingDistanceThreshold),
        api.scanDocumentLineages(path, docDistanceThreshold),
        api.scanCarvingCandidates(path),
      ]);

      let totalIssues = 0;

      if (collRes.status === "fulfilled") {
        setPairs(collRes.value);
        totalIssues += collRes.value.length;
      }
      if (rectRes.status === "fulfilled") {
        setRectifications(rectRes.value);
        setSelectedRectifications(new Set(rectRes.value.map((r) => r.current_path)));
        totalIssues += rectRes.value.length;
      }
      if (nearRes.status === "fulfilled") {
        setClusters(nearRes.value);
        totalIssues += nearRes.value.length;
      }
      if (docRes.status === "fulfilled") {
        setDocClusters(docRes.value);
        totalIssues += docRes.value.length;
      }
      if (carvRes.status === "fulfilled") {
        setCarvingProposals(carvRes.value);
        setSelectedCarvings(new Set(carvRes.value.map((c) => c.source_path)));
        totalIssues += carvRes.value.length;
      }

      setStatusMessage(
        `Comprehensive Deep Audit completed across all 5 engines. Discovered ${totalIssues} actionable items/clusters.`
      );
    } catch (err) {
      setStatusMessage(`Comprehensive deep audit encountered an error: ${String(err)}`);
    } finally {
      setLoadingAll(false);
    }
  };

  const handleScanCollisions = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanAndCompareRecoveryPairs(folderPath.trim());
      setPairs(res);
      if (res.length === 0) {
        setStatusMessage("Scan complete. No conflicting corrupted recovery pairs found in this folder.");
      }
    } catch (err) {
      setStatusMessage(`Recovery scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleScanRectifications = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanExtensionRectifications(folderPath.trim());
      setRectifications(res);
      setSelectedRectifications(new Set(res.map((r) => r.current_path)));
      if (res.length === 0) {
        setStatusMessage("No misidentified or unknown file signatures found in this folder.");
      }
    } catch (err) {
      setStatusMessage(`Signature scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExecuteRectifications = async () => {
    const toApply = rectifications.filter((r) => selectedRectifications.has(r.current_path));
    if (toApply.length === 0) return;
    setRectifying(true);
    try {
      const items: [string, string][] = toApply.map((r) => [r.current_path, r.suggested_new_path]);
      const res = await api.executeExtensionRectifications({ items });
      const succeeded = res.results.filter((r) => r.success).length;
      setStatusMessage(`Successfully rectified extensions for ${succeeded} file(s).`);
      handleScanRectifications();
    } catch (err) {
      setStatusMessage(`Failed to apply extension fixes: ${String(err)}`);
    } finally {
      setRectifying(false);
    }
  };

  const handleScanNearDuplicates = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanPerceptualDuplicates(folderPath.trim(), hammingDistanceThreshold);
      setClusters(res);
      if (res.length === 0) {
        setStatusMessage("Scan complete. No perceptual near-duplicate photos found within threshold.");
      }
    } catch (err) {
      setStatusMessage(`Near-duplicate scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handlePlanReconstruction = async (candA?: string, candB?: string) => {
    const a = candA || candidateAPath;
    const b = candB || candidateBPath;
    if (!a.trim() || !b.trim()) {
      setStatusMessage("Please specify paths for both Candidate A and Candidate B.");
      return;
    }
    setLoading(true);
    setStatusMessage(null);
    setReconstructionResult(null);
    try {
      const plan = await api.planFileReconstruction(a.trim(), b.trim());
      setReconstructionPlan(plan);
      setActiveTab("reconstruction");
      setCandidateAPath(a);
      setCandidateBPath(b);
    } catch (err) {
      setStatusMessage(`Failed to generate reconstruction plan: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExecuteReconstruction = async () => {
    if (!reconstructionPlan) return;
    setIsReconstructing(true);
    setStatusMessage(null);
    try {
      const res = await api.executeFileReconstruction({ plan: reconstructionPlan });
      setReconstructionResult(res);
      setStatusMessage(`Reconstruction successfully synthesized to ${res.output_path}! Health Score: ${res.resulting_health_score}/100.`);
    } catch (err) {
      setStatusMessage(`Reconstruction execution failed: ${String(err)}`);
    } finally {
      setIsReconstructing(false);
    }
  };

  const handleScanDocLineages = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanDocumentLineages(folderPath.trim(), docDistanceThreshold);
      setDocClusters(res);
      if (res.length === 0) {
        setStatusMessage("Scan complete. No document near-duplicate revisions or drafts found within threshold.");
      }
    } catch (err) {
      setStatusMessage(`Document lineage scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleQuarantineNearDupes = async (pathsToQuarantine: string[]) => {
    if (pathsToQuarantine.length === 0) return;
    setQuarantining(true);
    setStatusMessage(null);
    try {
      const res = await api.quarantineRawPaths(pathsToQuarantine);
      setStatusMessage(`Safely moved ${res.report.succeeded} file(s) into FileHarbor Quarantine (Batch: ${res.report.batch_id}). Files are protected and can be restored anytime.`);
      const quarantinedSet = new Set(pathsToQuarantine);
      setClusters((prev) =>
        prev
          .map((c) => ({
            ...c,
            members: c.members.filter((m) => !quarantinedSet.has(m.path)),
          }))
          .filter((c) => c.members.length > 1)
      );
    } catch (err) {
      setStatusMessage(`Quarantine failed: ${String(err)}`);
    } finally {
      setQuarantining(false);
    }
  };

  const handleQuarantineDrafts = async (pathsToQuarantine: string[]) => {
    if (pathsToQuarantine.length === 0) return;
    setQuarantining(true);
    setStatusMessage(null);
    try {
      const res = await api.quarantineRawPaths(pathsToQuarantine);
      setStatusMessage(`Safely moved ${res.report.succeeded} obsolete draft(s) into FileHarbor Quarantine (Batch: ${res.report.batch_id}). Protected and recoverable anytime.`);
      const quarantinedSet = new Set(pathsToQuarantine);
      setDocClusters((prev) =>
        prev
          .map((c) => ({
            ...c,
            members: c.members.filter((m) => !quarantinedSet.has(m.fingerprint.path)),
          }))
          .filter((c) => c.members.length > 1)
      );
    } catch (err) {
      setStatusMessage(`Quarantine failed: ${String(err)}`);
    } finally {
      setQuarantining(false);
    }
  };

  const handleScanCarvings = async () => {
    if (!folderPath.trim()) return;
    setLoading(true);
    setStatusMessage(null);
    try {
      const res = await api.scanCarvingCandidates(folderPath.trim());
      setCarvingProposals(res);
      setSelectedCarvings(new Set(res.map((p) => p.source_path)));
      if (res.length === 0) {
        setStatusMessage("No orphaned binary (.bin/.dat) or misidentified recovery files found in this folder.");
      }
    } catch (err) {
      setStatusMessage(`Carving scan failed: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExecuteCarvings = async () => {
    const toApply = carvingProposals.filter((p) => selectedCarvings.has(p.source_path));
    if (toApply.length === 0) return;
    setCarvingExecuting(true);
    try {
      const items: [string, string][] = toApply.map((p) => [p.source_path, p.proposed_path]);
      const res = await api.executeCarvingRenames({ items });
      setStatusMessage(`Successfully renamed ${res.succeeded} file(s) into canonical structured names (${res.failed} failed).`);
      handleScanCarvings();
    } catch (err) {
      setStatusMessage(`Carving rename execution failed: ${String(err)}`);
    } finally {
      setCarvingExecuting(false);
    }
  };

  const getRecommendationBadge = (rec: RecoveryPairComparison["recommendation"]) => {
    switch (rec) {
      case "KeepA":
        return <span className="keep-badge">Keep Candidate A</span>;
      case "KeepB":
        return <span className="keep-badge">Keep Candidate B</span>;
      case "BothHealthy":
        return <span className="badge" style={{ background: "rgba(56, 189, 248, 0.2)", color: "var(--accent)" }}>Both Healthy Copies</span>;
      case "BothCorrupted":
        return <span className="badge" style={{ background: "rgba(239, 68, 68, 0.2)", color: "var(--danger)" }}>Both Severely Corrupted</span>;
      case "Inconclusive":
      default:
        return <span className="badge" style={{ background: "rgba(245, 158, 11, 0.2)", color: "var(--warning)" }}>Manual Review Advised</span>;
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      <div className="section-header">
        <div>
          <h2>File Health & Recovery Intelligence</h2>
          <p className="page-intro">
            Understand file content integrity, resolve recovery collisions from damaged HDDs/SSDs,
            rectify misidentified file signatures (.bin/tmp), detect near-duplicate photos, and reconstruct clean files from damaged copies.
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="tabs" role="tablist" aria-label="File Health and Recovery Inspection Modes" style={{ display: "flex", gap: "8px" }}>
        <button
          role="tab"
          id="tab-collisions"
          aria-selected={activeTab === "collisions"}
          aria-controls="panel-collisions"
          className={activeTab === "collisions" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("collisions"); setStatusMessage(null); }}
        >
          Recovery Collisions
        </button>
        <button
          role="tab"
          id="tab-rectifier"
          aria-selected={activeTab === "rectifier"}
          aria-controls="panel-rectifier"
          className={activeTab === "rectifier" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("rectifier"); setStatusMessage(null); }}
        >
          Signature Rectifier (Unknown Files)
        </button>
        <button
          role="tab"
          id="tab-near-dupes"
          aria-selected={activeTab === "near-dupes"}
          aria-controls="panel-near-dupes"
          className={activeTab === "near-dupes" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("near-dupes"); setStatusMessage(null); }}
        >
          Near-Duplicate Photos (pHash)
        </button>
        <button
          role="tab"
          id="tab-reconstruction"
          aria-selected={activeTab === "reconstruction"}
          aria-controls="panel-reconstruction"
          className={activeTab === "reconstruction" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("reconstruction"); setStatusMessage(null); }}
        >
          Multi-Copy Reconstruction
        </button>
        <button
          role="tab"
          id="tab-doc-lineage"
          aria-selected={activeTab === "doc-lineage"}
          aria-controls="panel-doc-lineage"
          className={activeTab === "doc-lineage" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("doc-lineage"); setStatusMessage(null); }}
        >
          Document Lineage & Drafts (SimHash)
        </button>
        <button
          role="tab"
          id="tab-carving"
          aria-selected={activeTab === "carving"}
          aria-controls="panel-carving"
          className={activeTab === "carving" ? "btn btn-primary" : "btn btn-secondary"}
          onClick={() => { setActiveTab("carving"); setStatusMessage(null); }}
        >
          Deep Carving & EXIF Renamer
        </button>
      </div>

      {/* Directory Selector Card (for bulk scanners) */}
      {activeTab !== "reconstruction" && (
        <div className="card">
          <label htmlFor="health-recovery-target-dir" style={{ display: "block", fontSize: "13px", color: "var(--text-dim)", marginBottom: "8px" }}>
            Target Directory to Inspect:
          </label>
          <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
            <input
              id="health-recovery-target-dir"
              type="text"
              className="input"
              aria-label="Target directory to inspect"
              style={{
                flex: 1,
                padding: "8px 12px",
                background: "var(--bg)",
                color: "var(--text)",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius)",
              }}
              placeholder="Path to folder (e.g. C:\Recovered_Files or D:\Downloads)"
              value={folderPath}
              onChange={(e) => setFolderPath(e.target.value)}
            />
            <button
              type="button"
              onClick={handleBrowseFolder}
              className="btn btn-secondary"
            >
              Browse…
            </button>
            {activeTab === "collisions" && (
              <button
                onClick={handleScanCollisions}
                disabled={loading || !folderPath.trim()}
                className="btn btn-primary"
              >
                {loading ? "Inspecting Files..." : "Find Recovery Conflicts"}
              </button>
            )}
            {activeTab === "rectifier" && (
              <button
                onClick={handleScanRectifications}
                disabled={loading || !folderPath.trim()}
                className="btn btn-primary"
              >
                {loading ? "Scanning Signatures..." : "Detect Misidentified Files"}
              </button>
            )}
            {activeTab === "near-dupes" && (
              <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                <select
                  value={hammingDistanceThreshold}
                  onChange={(e) => setHammingDistanceThreshold(Number(e.target.value))}
                  style={{
                    padding: "8px",
                    background: "var(--bg)",
                    color: "var(--text)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                  }}
                >
                  <option value={4}>Very Strict (94%+ match)</option>
                  <option value={6}>Standard (90%+ match)</option>
                  <option value={10}>Broad (84%+ match)</option>
                </select>
                <button
                  onClick={handleScanNearDuplicates}
                  disabled={loading || !folderPath.trim()}
                  className="btn btn-primary"
                >
                  {loading ? "Hashing Images..." : "Find Near-Duplicates"}
                </button>
              </div>
            )}
            {activeTab === "doc-lineage" && (
              <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                <select
                  value={docDistanceThreshold}
                  onChange={(e) => setDocDistanceThreshold(Number(e.target.value))}
                  style={{
                    padding: "8px",
                    background: "var(--bg)",
                    color: "var(--text)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius)",
                  }}
                >
                  <option value={8}>Strict Revision (87%+ match)</option>
                  <option value={12}>Standard Drafts (81%+ match)</option>
                  <option value={16}>Broad Variants (75%+ match)</option>
                </select>
                <button
                  onClick={handleScanDocLineages}
                  disabled={loading || !folderPath.trim()}
                  className="btn btn-primary"
                >
                  {loading ? "Analyzing Documents..." : "Cluster Document Drafts"}
                </button>
              </div>
            )}
            {activeTab === "carving" && (
              <button
                onClick={handleScanCarvings}
                disabled={loading || !folderPath.trim()}
                className="btn btn-primary"
              >
                {loading ? "Carving Streams..." : "Carve & Extract Metadata"}
              </button>
            )}
            <button
              type="button"
              onClick={handleRunAllAudits}
              disabled={loading || loadingAll || !folderPath.trim()}
              className="btn btn-secondary"
              style={{
                borderColor: "var(--accent-deep-border)",
                color: "var(--accent-deep-text)",
                fontWeight: 600,
                whiteSpace: "nowrap",
              }}
              title="Runs all 5 health and recovery inspection engines concurrently on this folder"
            >
              {loadingAll ? "Running All Audits..." : "⚡ Run All Audits"}
            </button>
          </div>

          <div
            style={{
              marginTop: "12px",
              padding: "10px 14px",
              background: "var(--bg-secondary)",
              border: "1px solid var(--border)",
              borderRadius: "var(--radius-sm)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              flexWrap: "wrap",
              gap: "8px",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", flexWrap: "wrap" }}>
              <span style={{ fontWeight: 600, color: "var(--text)" }}>Audit Engines:</span>
              <button
                type="button"
                className="badge"
                style={{
                  cursor: "pointer",
                  border: "none",
                  background: pairs.length > 0 ? "rgba(239, 68, 68, 0.15)" : "var(--bg)",
                  color: pairs.length > 0 ? "var(--danger)" : "var(--text-muted)",
                }}
                onClick={() => setActiveTab("collisions")}
              >
                🩹 Collisions: {pairs.length}
              </button>
              <button
                type="button"
                className="badge"
                style={{
                  cursor: "pointer",
                  border: "none",
                  background: rectifications.length > 0 ? "rgba(245, 158, 11, 0.15)" : "var(--bg)",
                  color: rectifications.length > 0 ? "var(--warning)" : "var(--text-muted)",
                }}
                onClick={() => setActiveTab("rectifier")}
              >
                🏷️ Signatures: {rectifications.length}
              </button>
              <button
                type="button"
                className="badge"
                style={{
                  cursor: "pointer",
                  border: "none",
                  background: clusters.length > 0 ? "rgba(59, 130, 246, 0.15)" : "var(--bg)",
                  color: clusters.length > 0 ? "var(--focus)" : "var(--text-muted)",
                }}
                onClick={() => setActiveTab("near-dupes")}
              >
                🖼️ Photos (pHash): {clusters.length}
              </button>
              <button
                type="button"
                className="badge"
                style={{
                  cursor: "pointer",
                  border: "none",
                  background: docClusters.length > 0 ? "var(--accent-deep-subtle)" : "var(--bg)",
                  color: docClusters.length > 0 ? "var(--accent-deep-text)" : "var(--text-muted)",
                }}
                onClick={() => setActiveTab("doc-lineage")}
              >
                📄 Docs (SimHash): {docClusters.length}
              </button>
              <button
                type="button"
                className="badge"
                style={{
                  cursor: "pointer",
                  border: "none",
                  background: carvingProposals.length > 0 ? "rgba(16, 185, 129, 0.15)" : "var(--bg)",
                  color: carvingProposals.length > 0 ? "var(--success)" : "var(--text-muted)",
                }}
                onClick={() => setActiveTab("carving")}
              >
                🧩 Carved: {carvingProposals.length}
              </button>
            </div>
            <span className="hint" style={{ fontSize: "11px" }}>
              Click any engine pill to inspect findings
            </span>
          </div>
        </div>
      )}

      {statusMessage && (
        <div className="card notice-banner info" style={{ padding: "12px 16px" }}>
          {statusMessage}
        </div>
      )}

      {/* TAB 1: RECOVERY COLLISIONS */}
      {activeTab === "collisions" && pairs.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          <div style={{ fontSize: "14px", color: "var(--text-dim)" }}>
            Found {pairs.length} conflicting recovery candidate pair(s)
          </div>

          {pairs.map((pair) => (
            <div key={pair.pair_id} className="card" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid var(--border)", paddingBottom: "10px" }}>
                <div>
                  <h3 style={{ margin: 0 }}>{pair.base_name}</h3>
                  <div style={{ fontSize: "12px", color: "var(--text-dim)", marginTop: "4px" }}>
                    {pair.explanation}
                  </div>
                </div>
                <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                  {getRecommendationBadge(pair.recommendation)}
                  <button
                    className="btn btn-secondary btn-small"
                    style={{ fontWeight: 600, color: "var(--accent)" }}
                    onClick={() => handlePlanReconstruction(pair.candidate_a.file_path, pair.candidate_b.file_path)}
                  >
                    Reconstruct Healthy File
                  </button>
                </div>
              </div>

              {/* Candidates Comparison */}
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "16px" }}>
                {/* Candidate A */}
                <div
                  style={{
                    padding: "12px",
                    borderRadius: "var(--radius)",
                    border: pair.recommendation === "KeepA" ? "1px solid rgba(16, 185, 129, 0.4)" : "1px solid var(--border)",
                    background: pair.recommendation === "KeepA" ? "rgba(16, 185, 129, 0.05)" : "rgba(255, 255, 255, 0.02)",
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
                    <span style={{ fontWeight: 600, fontSize: "12px", color: "var(--text-dim)" }}>CANDIDATE A</span>
                    <span style={{ fontWeight: 700, fontSize: "12px", color: pair.candidate_a.health_score >= 70 ? "var(--success)" : "var(--warning)" }}>
                      Score: {pair.candidate_a.health_score}/100
                    </span>
                  </div>
                  <div className="mono" style={{ fontSize: "11px", wordBreak: "break-all", marginBottom: "8px" }}>
                    {pair.candidate_a.file_path}
                  </div>
                  <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", gap: "12px" }}>
                    <span>Size: {formatBytes(pair.candidate_a.file_size)}</span>
                    <span>Entropy: {pair.candidate_a.entropy.toFixed(2)}/8.0</span>
                    <span>Null Run: {pair.candidate_a.null_stats.null_percentage.toFixed(1)}%</span>
                  </div>
                  {pair.candidate_a.issues.length > 0 && (
                    <div style={{ marginTop: "8px", fontSize: "11px", color: "var(--danger)", display: "flex", flexDirection: "column", gap: "2px" }}>
                      {pair.candidate_a.issues.map((issue, idx) => (
                        <div key={idx} style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                          <IconAlert size={12} />
                          <span>{issue}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                {/* Candidate B */}
                <div
                  style={{
                    padding: "12px",
                    borderRadius: "var(--radius)",
                    border: pair.recommendation === "KeepB" ? "1px solid rgba(16, 185, 129, 0.4)" : "1px solid var(--border)",
                    background: pair.recommendation === "KeepB" ? "rgba(16, 185, 129, 0.05)" : "rgba(255, 255, 255, 0.02)",
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
                    <span style={{ fontWeight: 600, fontSize: "12px", color: "var(--text-dim)" }}>CANDIDATE B</span>
                    <span style={{ fontWeight: 700, fontSize: "12px", color: pair.candidate_b.health_score >= 70 ? "var(--success)" : "var(--warning)" }}>
                      Score: {pair.candidate_b.health_score}/100
                    </span>
                  </div>
                  <div className="mono" style={{ fontSize: "11px", wordBreak: "break-all", marginBottom: "8px" }}>
                    {pair.candidate_b.file_path}
                  </div>
                  <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", gap: "12px" }}>
                    <span>Size: {formatBytes(pair.candidate_b.file_size)}</span>
                    <span>Entropy: {pair.candidate_b.entropy.toFixed(2)}/8.0</span>
                    <span>Null Run: {pair.candidate_b.null_stats.null_percentage.toFixed(1)}%</span>
                  </div>
                  {pair.candidate_b.issues.length > 0 && (
                    <div style={{ marginTop: "8px", fontSize: "11px", color: "var(--danger)", display: "flex", flexDirection: "column", gap: "2px" }}>
                      {pair.candidate_b.issues.map((issue, idx) => (
                        <div key={idx} style={{ display: "flex", alignItems: "center", gap: "4px" }}>
                          <IconAlert size={12} />
                          <span>{issue}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* TAB 2: SIGNATURE RECTIFIER (UNKNOWN FILES) */}
      {activeTab === "rectifier" && rectifications.length > 0 && (
        <div className="card">
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "16px" }}>
            <div>
              <h3 style={{ margin: 0 }}>Misidentified & Unknown File Signatures</h3>
              <p style={{ margin: "4px 0 0 0", fontSize: "13px", color: "var(--text-dim)" }}>
                Detected {rectifications.length} file(s) whose internal magic-bytes disagree with their file extensions.
              </p>
            </div>
            <button
              className="btn btn-primary"
              onClick={handleExecuteRectifications}
              disabled={rectifying || selectedRectifications.size === 0}
            >
              {rectifying ? "Applying..." : `Rectify Selected (${selectedRectifications.size})`}
            </button>
          </div>

          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "13px" }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)", color: "var(--text-dim)" }}>
                  <th style={{ padding: "8px", width: "40px" }}>
                    <input
                      type="checkbox"
                      checked={selectedRectifications.size === rectifications.length}
                      aria-label="Select all extension rectification proposals"
                      onChange={(e) => {
                        if (e.target.checked) setSelectedRectifications(new Set(rectifications.map((r) => r.current_path)));
                        else setSelectedRectifications(new Set());
                      }}
                    />
                  </th>
                  <th style={{ padding: "8px", textAlign: "left" }}>File Name</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Current Extension</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Detected True Format</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Proposed Fix</th>
                  <th style={{ padding: "8px", textAlign: "right" }}>Confidence</th>
                </tr>
              </thead>
              <tbody>
                {rectifications.map((r) => {
                  const isChecked = selectedRectifications.has(r.current_path);
                  return (
                    <tr key={r.current_path} style={{ borderBottom: "1px solid var(--border)" }}>
                      <td style={{ padding: "10px", textAlign: "center" }}>
                        <input
                          type="checkbox"
                          checked={isChecked}
                          aria-label={`Select rectification for ${r.file_name}`}
                          onChange={() => {
                            const next = new Set(selectedRectifications);
                            if (next.has(r.current_path)) next.delete(r.current_path);
                            else next.add(r.current_path);
                            setSelectedRectifications(next);
                          }}
                        />
                      </td>
                      <td style={{ padding: "10px", fontWeight: 600 }}>{r.file_name}</td>
                      <td style={{ padding: "10px", color: "var(--danger)" }}>.{r.current_extension || "(none)"}</td>
                      <td style={{ padding: "10px", color: "var(--accent)" }}>{r.detected_format}</td>
                      <td style={{ padding: "10px", fontWeight: 600, color: "var(--success)" }}>
                        .{r.suggested_extension}
                      </td>
                      <td style={{ padding: "10px", textAlign: "right", color: "var(--text-dim)" }}>
                        {r.confidence_percentage}%
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* TAB 3: NEAR-DUPLICATE PHOTOS */}
      {activeTab === "near-dupes" && clusters.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          {(() => {
            const allRedundantPhotos = clusters.flatMap((c) =>
              c.members.filter((m) => m.path !== c.primary_keep_path).map((m) => m.path)
            );
            return (
              <div className="card" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px 16px" }}>
                <div>
                  <h4 style={{ margin: 0 }}>Found {clusters.length} visual near-duplicate photo cluster(s)</h4>
                  <p style={{ margin: "2px 0 0 0", fontSize: "12px", color: "var(--text-dim)" }}>
                    {allRedundantPhotos.length} lower-resolution or redundant duplicate copies identified.
                  </p>
                </div>
                <button
                  className="btn btn-secondary"
                  style={{ borderColor: "rgba(239, 68, 68, 0.4)", color: "var(--danger)", display: "inline-flex", alignItems: "center", gap: "6px" }}
                  onClick={() => handleQuarantineNearDupes(allRedundantPhotos)}
                  disabled={quarantining || allRedundantPhotos.length === 0}
                >
                  <IconTrash size={14} />
                  <span>{quarantining ? "Quarantining..." : `Quarantine All Redundant (${allRedundantPhotos.length})`}</span>
                </button>
              </div>
            );
          })()}

          {clusters.map((cluster) => {
            const redundantInCluster = cluster.members
              .filter((m) => m.path !== cluster.primary_keep_path)
              .map((m) => m.path);

            return (
              <div key={cluster.cluster_id} className="card" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid var(--border)", paddingBottom: "8px" }}>
                  <div>
                    <h3 style={{ margin: 0, display: "flex", alignItems: "center", gap: "8px" }}>
                      <span>Cluster: {cluster.members[0].file_name}</span>
                      <span className="badge" style={{ background: "rgba(56, 189, 248, 0.15)", color: "var(--accent)" }}>
                        {cluster.similarity_percentage.toFixed(1)}% Visual Match
                      </span>
                    </h3>
                    <p style={{ margin: "4px 0 0 0", fontSize: "12px", color: "var(--text-dim)" }}>
                      {cluster.explanation}
                    </p>
                  </div>
                  {redundantInCluster.length > 0 && (
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: "12px", padding: "4px 10px", borderColor: "rgba(239, 68, 68, 0.3)", color: "var(--danger)" }}
                      onClick={() => handleQuarantineNearDupes(redundantInCluster)}
                      disabled={quarantining}
                    >
                      Quarantine Cluster Copies ({redundantInCluster.length})
                    </button>
                  )}
                </div>

                {/* Member Visual Grid */}
                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: "14px" }}>
                  {cluster.members.map((m) => {
                    const isKeep = m.path === cluster.primary_keep_path;
                    return (
                      <div
                        key={m.path}
                        style={{
                          padding: "14px",
                          borderRadius: "var(--radius)",
                          border: isKeep ? "1px solid rgba(34, 197, 94, 0.4)" : "1px solid var(--border)",
                          background: isKeep ? "rgba(34, 197, 94, 0.04)" : "rgba(255, 255, 255, 0.02)",
                          display: "flex",
                          flexDirection: "column",
                          gap: "10px",
                        }}
                      >
                        {/* Top Status & Action Header */}
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px" }}>
                          {isKeep ? (
                            <span className="keep-badge" style={{ fontSize: "11px", whiteSpace: "nowrap" }}>
                              ⭐ Highest Quality
                            </span>
                          ) : (
                            <span className="badge" style={{ background: "rgba(239, 68, 68, 0.15)", color: "var(--danger)", fontSize: "11px", whiteSpace: "nowrap" }}>
                              Redundant
                            </span>
                          )}
                          <button
                            type="button"
                            className="btn btn-ghost btn-small"
                            style={{ fontSize: "11px", padding: "2px 8px" }}
                            onClick={() => api.openFile(m.path)}
                            title="Open in system photo viewer"
                          >
                            Open File ↗
                          </button>
                        </div>

                        {/* Interactive Image Preview Thumbnail */}
                        <div
                          onClick={() => api.openFile(m.path)}
                          title="Click to view full image in default photo viewer"
                          style={{
                            width: "100%",
                            height: "150px",
                            borderRadius: "6px",
                            overflow: "hidden",
                            background: "rgba(0, 0, 0, 0.25)",
                            border: "1px solid var(--border)",
                            cursor: "pointer",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            position: "relative",
                          }}
                        >
                          <img
                            src={convertFileSrc(m.path)}
                            alt={m.file_name}
                            style={{
                              maxWidth: "100%",
                              maxHeight: "100%",
                              objectFit: "contain",
                            }}
                            onError={(e) => {
                              (e.target as HTMLElement).style.display = "none";
                            }}
                          />
                          <div
                            style={{
                              position: "absolute",
                              bottom: "6px",
                              right: "6px",
                              background: "rgba(0, 0, 0, 0.75)",
                              color: "var(--text-bright)",
                              fontSize: "10px",
                              padding: "2px 6px",
                              borderRadius: "4px",
                              pointerEvents: "none",
                            }}
                          >
                            Click to View ↗
                          </div>
                        </div>

                        {/* Filename and Metadata */}
                        <div>
                          <div
                            style={{
                              fontWeight: 600,
                              fontSize: "13px",
                              color: "var(--text-bright)",
                              wordBreak: "break-all",
                              marginBottom: "4px",
                            }}
                            title={m.file_name}
                          >
                            {m.file_name}
                          </div>
                          <div
                            className="mono"
                            style={{
                              fontSize: "11px",
                              color: "var(--text-dim)",
                              wordBreak: "break-all",
                              marginBottom: "8px",
                            }}
                            title={m.path}
                          >
                            {m.path}
                          </div>
                          <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", flexDirection: "column", gap: "2px" }}>
                            <span>Resolution: {m.width && m.height ? `${m.width} × ${m.height}` : "Unknown"}</span>
                            <span>Size: {formatBytes(m.file_size)}</span>
                            <span>Format: {m.format}</span>
                          </div>
                        </div>

                        {/* Bottom Actions */}
                        {!isKeep && (
                          <div style={{ marginTop: "auto", paddingTop: "8px", display: "flex", justifyContent: "flex-end" }}>
                            <button
                              className="btn btn-secondary"
                              style={{ fontSize: "11px", padding: "4px 10px", borderColor: "rgba(239, 68, 68, 0.4)", color: "var(--danger)" }}
                              onClick={() => handleQuarantineNearDupes([m.path])}
                              disabled={quarantining}
                            >
                              Quarantine
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* TAB 4: MULTI-COPY FILE RECONSTRUCTION */}
      {activeTab === "reconstruction" && (
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          <div className="card">
            <h3>Multi-Copy Sector Chunk Stitching</h3>
            <p style={{ fontSize: "13px", color: "var(--text-dim)", margin: "4px 0 16px 0" }}>
              Combine undamaged 4KB blocks from two damaged copies of the same file to synthesize a 100% repaired version.
            </p>

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px", marginBottom: "16px" }}>
              <div>
                <label htmlFor="candidate-copy-a-path" style={{ display: "block", fontSize: "12px", color: "var(--text-dim)", marginBottom: "4px" }}>
                  Candidate Copy A:
                </label>
                <div style={{ display: "flex", gap: "8px" }}>
                  <input
                    id="candidate-copy-a-path"
                    type="text"
                    value={candidateAPath}
                    onChange={(e) => setCandidateAPath(e.target.value)}
                    placeholder="Path to damaged Copy A..."
                    aria-label="Path to damaged Candidate Copy A"
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
                    type="button"
                    className="btn btn-secondary btn-small"
                    onClick={handleBrowseCandidateA}
                  >
                    Browse…
                  </button>
                </div>
              </div>
              <div>
                <label htmlFor="candidate-copy-b-path" style={{ display: "block", fontSize: "12px", color: "var(--text-dim)", marginBottom: "4px" }}>
                  Candidate Copy B:
                </label>
                <div style={{ display: "flex", gap: "8px" }}>
                  <input
                    id="candidate-copy-b-path"
                    type="text"
                    value={candidateBPath}
                    onChange={(e) => setCandidateBPath(e.target.value)}
                    placeholder="Path to damaged Copy B..."
                    aria-label="Path to damaged Candidate Copy B"
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
                    type="button"
                    className="btn btn-secondary btn-small"
                    onClick={handleBrowseCandidateB}
                  >
                    Browse…
                  </button>
                </div>
              </div>
            </div>

            <button
              className="btn btn-primary"
              disabled={loading || !candidateAPath.trim() || !candidateBPath.trim()}
              onClick={() => handlePlanReconstruction()}
            >
              {loading ? "Analyzing Blocks..." : "Analyze & Plan Reconstruction"}
            </button>
          </div>

          {/* Reconstruction Plan & Heatmap */}
          {reconstructionPlan && (
            <div className="card" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <h3 style={{ margin: 0 }}>Reconstruction Plan Assessment</h3>
                  <div style={{ fontSize: "13px", color: "var(--text-dim)", marginTop: "4px" }}>
                    Target File Size: <strong>{formatBytes(reconstructionPlan.file_size)}</strong> ({reconstructionPlan.block_map.total_chunks} blocks of 4KB)
                  </div>
                </div>
                <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                  <span
                    className="badge"
                    style={{
                      background: reconstructionPlan.can_achieve_full_repair ? "rgba(16, 185, 129, 0.15)" : "rgba(245, 158, 11, 0.15)",
                      color: reconstructionPlan.can_achieve_full_repair ? "var(--success)" : "var(--warning)",
                      fontSize: "13px",
                      padding: "6px 12px",
                    }}
                  >
                    {reconstructionPlan.potential_recovery_percentage.toFixed(1)}% Repairable
                  </span>
                  <button
                    className="btn btn-primary"
                    disabled={isReconstructing}
                    onClick={handleExecuteReconstruction}
                  >
                    {isReconstructing ? "Synthesizing..." : "Synthesize Reconstructed File"}
                  </button>
                </div>
              </div>

              {/* Stats Bar */}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: "8px", background: "rgba(0,0,0,0.15)", padding: "12px", borderRadius: "var(--radius)" }}>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--text-dim)" }}>Identical Blocks</div>
                  <div style={{ fontWeight: 600, color: "var(--text)" }}>{reconstructionPlan.block_map.identical_chunks}</div>
                </div>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--success)" }}>Rescued from A</div>
                  <div style={{ fontWeight: 600, color: "var(--success)" }}>{reconstructionPlan.block_map.chunks_from_a}</div>
                </div>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--accent)" }}>Rescued from B</div>
                  <div style={{ fontWeight: 600, color: "var(--accent)" }}>{reconstructionPlan.block_map.chunks_from_b}</div>
                </div>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--danger)" }}>Unrecoverable Holes</div>
                  <div style={{ fontWeight: 600, color: "var(--danger)" }}>{reconstructionPlan.block_map.unrecoverable_chunks}</div>
                </div>
                <div>
                  <div style={{ fontSize: "11px", color: "var(--warning)" }}>Conflicting Blocks</div>
                  <div style={{ fontWeight: 600, color: "var(--warning)" }}>{reconstructionPlan.block_map.conflicting_chunks}</div>
                </div>
              </div>

              {/* Visual Sector Map (First 120 blocks preview) */}
              <div>
                <div style={{ fontSize: "12px", color: "var(--text-dim)", marginBottom: "6px" }}>
                  Sector Block Heatmap (First {Math.min(120, reconstructionPlan.block_map.total_chunks)} chunks):
                </div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: "3px" }}>
                  {reconstructionPlan.block_map.block_sources.slice(0, 120).map((source, idx) => {
                    let color = "var(--text-muted)"; // identical
                    if (source === "SourceA") color = "var(--success)"; // rescued from A
                    if (source === "SourceB") color = "var(--accent)"; // rescued from B
                    if (source === "UnrecoverableNull") color = "var(--danger)"; // dead hole
                    if (source === "ConflictDifferentData") color = "var(--warning)"; // conflict

                    return (
                      <div
                        key={idx}
                        title={`Block #${idx}: ${source}`}
                        style={{
                          width: "12px",
                          height: "12px",
                          borderRadius: "2px",
                          background: color,
                        }}
                      />
                    );
                  })}
                </div>
                <div style={{ display: "flex", gap: "16px", marginTop: "8px", fontSize: "11px", color: "var(--text-dim)" }}>
                  <span style={{ display: "flex", alignItems: "center", gap: "4px" }}><span style={{ width: "8px", height: "8px", background: "var(--text-muted)", borderRadius: "1px" }}/> Both Identical</span>
                  <span style={{ display: "flex", alignItems: "center", gap: "4px" }}><span style={{ width: "8px", height: "8px", background: "var(--success)", borderRadius: "1px" }}/> Spliced from Copy A</span>
                  <span style={{ display: "flex", alignItems: "center", gap: "4px" }}><span style={{ width: "8px", height: "8px", background: "var(--accent)", borderRadius: "1px" }}/> Spliced from Copy B</span>
                  <span style={{ display: "flex", alignItems: "center", gap: "4px" }}><span style={{ width: "8px", height: "8px", background: "var(--danger)", borderRadius: "1px" }}/> Zero Hole in Both</span>
                </div>
              </div>

              {/* Destination Output Path */}
              <div style={{ fontSize: "12px", color: "var(--text-dim)" }}>
                Output Target: <span className="mono" style={{ color: "var(--text)" }}>{reconstructionPlan.output_path}</span>
              </div>
            </div>
          )}

          {/* Success Result Card */}
          {reconstructionResult && (
            <div className="card notice-banner success" style={{ padding: "16px" }}>
              <h4 style={{ margin: "0 0 8px 0", display: "flex", alignItems: "center", gap: "6px" }}>
                <IconCheck size={16} />
                <span>File Successfully Reconstructed!</span>
              </h4>
              <div style={{ fontSize: "13px", display: "flex", flexDirection: "column", gap: "4px" }}>
                <div>Synthesized Path: <span className="mono">{reconstructionResult.output_path}</span></div>
                <div>Bytes Written: <strong>{formatBytes(reconstructionResult.bytes_written)}</strong></div>
                <div>Health Score: <strong>{reconstructionResult.resulting_health_score} / 100</strong></div>
                <div>SHA-256: <span className="mono" style={{ fontSize: "11px" }}>{reconstructionResult.sha256}</span></div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* TAB 5: DOCUMENT LINEAGE & REVISIONS */}
      {activeTab === "doc-lineage" && docClusters.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          {(() => {
            const allObsoleteDrafts = docClusters.flatMap((c) =>
              c.members.filter((m) => m.role === "ObsoleteDraft").map((m) => m.fingerprint.path)
            );
            return (
              <div className="card" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px 16px" }}>
                <div>
                  <h4 style={{ margin: 0 }}>Found {docClusters.length} document version lineage cluster(s)</h4>
                  <p style={{ margin: "2px 0 0 0", fontSize: "12px", color: "var(--text-dim)" }}>
                    {allObsoleteDrafts.length} obsolete intermediate drafts identified across versions.
                  </p>
                </div>
                <button
                  className="btn btn-secondary"
                  style={{ borderColor: "rgba(245, 158, 11, 0.4)", color: "var(--warning)", display: "inline-flex", alignItems: "center", gap: "6px" }}
                  onClick={() => handleQuarantineDrafts(allObsoleteDrafts)}
                  disabled={quarantining || allObsoleteDrafts.length === 0}
                >
                  <IconTrash size={14} />
                  <span>{quarantining ? "Quarantining..." : `Quarantine All Obsolete Drafts (${allObsoleteDrafts.length})`}</span>
                </button>
              </div>
            );
          })()}

          {docClusters.map((cluster) => {
            const obsoleteInCluster = cluster.members
              .filter((m) => m.role === "ObsoleteDraft")
              .map((m) => m.fingerprint.path);

            return (
              <div key={cluster.cluster_id} className="card" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid var(--border)", paddingBottom: "8px" }}>
                  <div>
                    <h3 style={{ margin: 0, display: "flex", alignItems: "center", gap: "8px" }}>
                      <span>{cluster.members[0].fingerprint.file_name}</span>
                      <span className="badge" style={{ background: "rgba(16, 185, 129, 0.15)", color: "var(--success)" }}>
                        {cluster.similarity_percentage.toFixed(1)}% Vocabulary Match
                      </span>
                    </h3>
                    <p style={{ margin: "4px 0 0 0", fontSize: "12px", color: "var(--text-dim)" }}>
                      {cluster.explanation}
                    </p>
                  </div>
                  {obsoleteInCluster.length > 0 && (
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: "12px", padding: "4px 10px", borderColor: "rgba(245, 158, 11, 0.4)", color: "var(--warning)" }}
                      onClick={() => handleQuarantineDrafts(obsoleteInCluster)}
                      disabled={quarantining}
                    >
                      Quarantine Cluster Drafts ({obsoleteInCluster.length})
                    </button>
                  )}
                </div>

                {/* Members Lineage Progression */}
                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: "12px" }}>
                  {cluster.members.map((m) => {
                    const isFinal = m.role === "FinalVersion";
                    const isObsolete = m.role === "ObsoleteDraft";

                    return (
                      <div
                        key={m.fingerprint.path}
                        style={{
                          padding: "12px",
                          background: isFinal ? "rgba(16, 185, 129, 0.05)" : "var(--bg)",
                          border: isFinal ? "2px solid var(--success)" : isObsolete ? "1px solid rgba(245, 158, 11, 0.3)" : "1px solid var(--border)",
                          borderRadius: "var(--radius)",
                          display: "flex",
                          flexDirection: "column",
                          justifyContent: "space-between",
                          gap: "6px",
                        }}
                      >
                        <div>
                          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                            <span style={{ fontWeight: 600, fontSize: "13px" }}>{m.fingerprint.file_name}</span>
                            {isFinal ? (
                              <span className="badge" style={{ background: "rgba(16, 185, 129, 0.2)", color: "var(--success)" }}>
                                ⭐ Final Version
                              </span>
                            ) : isObsolete ? (
                              <span className="badge" style={{ background: "rgba(245, 158, 11, 0.15)", color: "var(--warning)" }}>
                                Obsolete Draft
                              </span>
                            ) : (
                              <span className="badge" style={{ background: "rgba(255, 255, 255, 0.05)", color: "var(--text-muted)" }}>
                                Variant
                              </span>
                            )}
                          </div>

                          <div className="mono" style={{ fontSize: "11px", color: "var(--text-dim)", wordBreak: "break-all" }}>
                            {m.fingerprint.path}
                          </div>

                          <div style={{ fontSize: "12px", color: "var(--text-dim)", display: "flex", flexDirection: "column", gap: "2px", marginTop: "4px" }}>
                            <span>Words: {m.fingerprint.word_count.toLocaleString()}</span>
                            <span>Size: {formatBytes(m.fingerprint.file_size)}</span>
                            <span>Format: {m.fingerprint.format}</span>
                            <span>Match to Final: {m.similarity_to_final.toFixed(1)}%</span>
                          </div>
                        </div>

                        {isObsolete && (
                          <div style={{ marginTop: "10px", display: "flex", justifyContent: "flex-end" }}>
                            <button
                              className="btn btn-secondary"
                              style={{ fontSize: "11px", padding: "2px 8px" }}
                              onClick={() => handleQuarantineDrafts([m.fingerprint.path])}
                              disabled={quarantining}
                            >
                              Quarantine Draft
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* TAB 6: DEEP CARVING & METADATA RENAMER */}
      {activeTab === "carving" && carvingProposals.length > 0 && (
        <div className="card">
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "16px" }}>
            <div>
              <h3 style={{ margin: 0 }}>Deep Stream Carving & Autonomous Metadata Renamer</h3>
              <p style={{ margin: "4px 0 0 0", fontSize: "13px", color: "var(--text-dim)" }}>
                Detected {carvingProposals.length} file(s) with extracted EXIF camera/timestamps, ID3 audio tags, or PDF headers.
              </p>
            </div>
            <button
              className="btn btn-primary"
              onClick={handleExecuteCarvings}
              disabled={carvingExecuting || selectedCarvings.size === 0}
            >
              {carvingExecuting ? "Renaming..." : `Apply Canonical Renames (${selectedCarvings.size})`}
            </button>
          </div>

          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "13px" }}>
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border)", color: "var(--text-dim)" }}>
                  <th style={{ padding: "8px", width: "40px" }}>
                    <input
                      type="checkbox"
                      checked={selectedCarvings.size === carvingProposals.length}
                      aria-label="Select all carving proposals"
                      onChange={(e) => {
                        if (e.target.checked) setSelectedCarvings(new Set(carvingProposals.map((p) => p.source_path)));
                        else setSelectedCarvings(new Set());
                      }}
                    />
                  </th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Current Name</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Detected Format</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Extracted Metadata</th>
                  <th style={{ padding: "8px", textAlign: "left" }}>Proposed Structured Name</th>
                  <th style={{ padding: "8px", textAlign: "right" }}>Confidence</th>
                </tr>
              </thead>
              <tbody>
                {carvingProposals.map((p) => {
                  const isChecked = selectedCarvings.has(p.source_path);
                  return (
                    <tr key={p.source_path} style={{ borderBottom: "1px solid var(--border)" }}>
                      <td style={{ padding: "10px" }}>
                        <input
                          type="checkbox"
                          checked={isChecked}
                          aria-label={`Select carving proposal for ${p.original_name}`}
                          onChange={(e) => {
                            const next = new Set(selectedCarvings);
                            if (e.target.checked) next.add(p.source_path);
                            else next.delete(p.source_path);
                            setSelectedCarvings(next);
                          }}
                        />
                      </td>
                      <td style={{ padding: "10px" }}>
                        <div style={{ fontWeight: 600 }}>{p.original_name}</div>
                        <div className="mono" style={{ fontSize: "11px", color: "var(--text-dim)" }}>{p.source_path}</div>
                      </td>
                      <td style={{ padding: "10px" }}>
                        <span className="badge" style={{ background: "rgba(56, 189, 248, 0.15)", color: "var(--accent)" }}>
                          {p.detected_extension.toUpperCase()} ({p.detected_mime})
                        </span>
                      </td>
                      <td style={{ padding: "10px", fontSize: "12px", color: "var(--text-dim)" }}>
                        {p.extracted_camera_or_artist && <div>Artist / Camera: {p.extracted_camera_or_artist}</div>}
                        {p.extracted_date && <div>Date: {p.extracted_date}</div>}
                        {p.extracted_title && <div>Title: {p.extracted_title}</div>}
                        {!p.extracted_camera_or_artist && !p.extracted_date && !p.extracted_title && <span>Magic signature only</span>}
                      </td>
                      <td style={{ padding: "10px" }}>
                        <span className="mono" style={{ color: "var(--success)", fontWeight: 600 }}>
                          {p.proposed_name}
                        </span>
                      </td>
                      <td style={{ padding: "10px", textAlign: "right", color: "var(--text-dim)" }}>
                        {p.confidence_score}%
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
};
