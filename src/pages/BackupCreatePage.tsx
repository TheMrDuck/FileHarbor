import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import type { BackupSchedule, RetentionPolicy, VerificationMode, DestinationValidation } from "../types";

type Step = "source" | "destination" | "review" | "running" | "complete";

interface Props {
  onCancel: () => void;
  onNavigate?: (page: string, opts?: { setId?: number; runId?: number }) => void;
}

export default function BackupCreatePage({ onCancel, onNavigate }: Props) {
  const [step, setStep] = useState<Step>("source");
  const [sourcePath, setSourcePath] = useState("");
  const [destinationPath, setDestinationPath] = useState("");
  const [schedule] = useState<BackupSchedule>("manual");
  const [retentionPolicy] = useState<RetentionPolicy>("keep_all");
  const [verificationMode] = useState<VerificationMode>("standard");
  const [validation, setValidation] = useState<DestinationValidation | null>(null);
  const [validating, setValidating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleSourceSelect = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      // Check if the selected folder is a backup destination
      const destId = await api.checkBackupDestinationSafety(selected);
      if (destId) {
        setError(`Warning: This folder is a FileHarbor backup destination (ID: ${destId}). Backing up a backup destination is not recommended.`);
      } else {
        setError(null);
      }
      setSourcePath(selected);
    }
  };

  const handleDestinationSelect = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setDestinationPath(selected);
      setError(null);
      validateDestination();
    }
  };

  const validateDestination = async () => {
    if (!sourcePath || !destinationPath) return;
    setValidating(true);
    try {
      const val = await api.validateBackupDestination(sourcePath, destinationPath);
      setValidation(val);
      if (!val.valid) {
        setError(val.errors.join(", "));
      } else {
        setError(null);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setValidating(false);
    }
  };

  const handleCreate = async () => {
    if (!sourcePath || !destinationPath) return;
    setCreating(true);
    setError(null);
    try {
      // First create the backup set
      const backupSet = await api.createBackupSet({
        name: sourcePath.split(/[\\/]/).pop() || "Backup",
        source_path: sourcePath,
        destination_path: destinationPath,
        schedule,
        retention_policy: retentionPolicy,
        verification_mode: verificationMode,
      });
      
      // Navigate to backup details page
      if (onNavigate) {
        onNavigate("backup-details", { setId: backupSet.id });
      } else {
        onCancel();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const canProceed = () => {
    switch (step) {
      case "source": return sourcePath.length > 0;
      case "destination": return destinationPath.length > 0 && validation?.valid;
      case "review": return true;
      default: return false;
    }
  };

  const renderStepIcon = (id: string) => {
    switch (id) {
      case "source":
        return (
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
          </svg>
        );
      case "destination":
        return (
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <rect width="20" height="8" x="2" y="4" rx="2" />
            <rect width="20" height="8" x="2" y="14" rx="2" />
            <line x1="6" y1="8" x2="6.01" y2="8" />
            <line x1="6" y1="18" x2="6.01" y2="18" />
          </svg>
        );
      case "review":
        return (
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
            <rect width="8" height="4" x="8" y="2" rx="1" ry="1" />
            <path d="m9 14 2 2 4-4" />
          </svg>
        );
      default:
        return null;
    }
  };

  const steps = [
    { id: "source", label: "Source" },
    { id: "destination", label: "Destination" },
    { id: "review", label: "Review" },
  ];

  return (
    <section className="wizard">
      <header className="wizard-header">
        <h2>Create Backup</h2>
        <button className="btn btn-ghost" onClick={onCancel}>Cancel</button>
      </header>

      <nav className="wizard-steps" aria-label="Wizard steps">
        {steps.map((s) => (
          <button
            key={s.id}
            className={`wizard-step ${step === s.id ? "active" : ""} ${["source", "destination", "review"].indexOf(s.id) < ["source", "destination", "review"].indexOf(step) ? "completed" : ""}`}
            onClick={() => ["source", "destination", "review"].indexOf(s.id) <= ["source", "destination", "review"].indexOf(step) && setStep(s.id as Step)}
            disabled={["source", "destination", "review"].indexOf(s.id) > ["source", "destination", "review"].indexOf(step)}
          >
            <span className="wizard-step-icon">{renderStepIcon(s.id)}</span>
            <span className="wizard-step-label">{s.label}</span>
          </button>
        ))}
      </nav>

      {error && <p className="form-error" role="alert">{error}</p>}

      {step === "source" && (
        <div className="wizard-content">
          <h3>What do you want to protect?</h3>
          <p className="page-intro">Select the folder containing the files you want to back up.</p>
          <div className="card">
            <div className="row">
              <input
                type="text"
                className="mono"
                value={sourcePath}
                readOnly
                placeholder="No folder selected"
                aria-label="Source backup folder path"
              />
              <button className="btn btn-secondary" onClick={handleSourceSelect}>
                Choose Folder…
              </button>
            </div>
            {sourcePath && <p className="mono small">{sourcePath}</p>}
            {error && <p className="form-error" role="alert">{error}</p>}
          </div>
          <div className="wizard-actions">
            <button className="btn btn-primary" onClick={() => setStep("destination")} disabled={!canProceed()}>
              Next
            </button>
          </div>
        </div>
      )}

      {step === "destination" && (
        <div className="wizard-content">
          <h3>Where should FileHarbor keep the backup?</h3>
          <p className="page-intro">Select a different drive or folder. The backup will be stored in a managed structure.</p>
          <div className="card">
            <div className="row">
              <input
                type="text"
                className="mono"
                value={destinationPath}
                readOnly
                placeholder="No folder selected"
                aria-label="Destination backup folder path"
              />
              <button className="btn btn-secondary" onClick={handleDestinationSelect} disabled={validating}>
                {validating ? "Validating…" : "Choose Folder…"}
              </button>
            </div>
            {destinationPath && <p className="mono small">{destinationPath}</p>}
            {validation && (
              <div className={`validation-result ${validation.valid ? "valid" : "invalid"}`}>
                <h4>{validation.valid ? "✓ Destination is valid" : "✗ Destination has issues"}</h4>
                {validation.errors.length > 0 && (
                  <ul className="errors">
                    {validation.errors.map((e, i) => <li key={i}>{e}</li>)}
                  </ul>
                )}
                {validation.warnings.length > 0 && (
                  <ul className="warnings">
                    {validation.warnings.map((w, i) => <li key={i}>{w}</li>)}
                  </ul>
                )}
                {validation.available_space_bytes !== null && (
                  <p className="space-info">Available space: {(validation.available_space_bytes / (1024 * 1024 * 1024)).toFixed(2)} GB</p>
                )}
              </div>
            )}
          </div>
          <div className="wizard-actions">
            <button className="btn btn-secondary" onClick={() => setStep("source")}>Back</button>
            <button className="btn btn-primary" onClick={() => setStep("review")} disabled={!canProceed()}>
              Next
            </button>
          </div>
        </div>
      )}

      {step === "review" && (
        <div className="wizard-content">
          <h3>Review your backup</h3>
          <div className="card">
            <dl className="review-list">
              <dt>Name</dt>
              <dd className="mono">{sourcePath.split(/[\\/]/).pop() || "Backup"}</dd>
              <dt>Source</dt>
              <dd className="truncated mono">{sourcePath}</dd>
              <dt>Destination</dt>
              <dd className="truncated mono">{destinationPath}</dd>
              <dt>Schedule</dt>
              <dd><span className="badge">{schedule}</span></dd>
              <dt>Retention</dt>
              <dd>{retentionPolicy}</dd>
              <dt>Verification</dt>
              <dd>{verificationMode}</dd>
            </dl>
          </div>
          <div className="wizard-actions">
            <button className="btn btn-secondary" onClick={() => setStep("destination")}>Back</button>
            <button className="btn btn-primary" onClick={handleCreate} disabled={creating}>
              {creating ? "Creating…" : "Create Backup"}
            </button>
          </div>
        </div>
      )}

      {step === "running" && (
        <div className="wizard-content running">
          <h3>Creating backup set…</h3>
          <div className="spinner" />
          <p>Please wait while we set up your backup.</p>
        </div>
      )}

      {step === "complete" && (
        <div className="wizard-content complete">
          <h3>✓ Backup created successfully!</h3>
          <p>Your backup set is ready. You can start your first backup from the details page.</p>
          <div className="wizard-actions">
            <button
              className="btn btn-primary"
              onClick={() => (onNavigate ? onNavigate("backups") : onCancel())}
            >
              View Backups
            </button>
          </div>
        </div>
      )}
    </section>
  );
}