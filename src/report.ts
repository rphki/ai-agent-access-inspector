// Derived information about a scan report, independent of the UI.
import type { Diagnostic, Finding, ScanReport, Severity, Warning } from "./types/report";

export function allFindings(report: ScanReport): Finding[] {
  return report.results.flatMap((result) => result.findings);
}

export function allDiagnostics(report: ScanReport): Diagnostic[] {
  return report.results.flatMap((result) => result.diagnostics);
}

export function warningCount(report: ScanReport): number {
  return allFindings(report).reduce((sum, finding) => sum + finding.warnings.length, 0);
}

const SEVERITY_ORDER: Severity[] = ["info", "low", "medium", "high"];

export function maxSeverity(warnings: Warning[]): Severity | null {
  let max: Severity | null = null;
  for (const warning of warnings) {
    if (max === null || SEVERITY_ORDER.indexOf(warning.severity) > SEVERITY_ORDER.indexOf(max)) {
      max = warning.severity;
    }
  }
  return max;
}

const UNREADABLE_CODES = ["unreadableFile", "fileTooLarge", "notARegularFile"];
const INVALID_CODES = ["invalidJson", "unexpectedType"];

export interface ReportState {
  /** `noAgent`: nothing detected; `noConfig`: detected, but no findings. */
  kind: "noAgent" | "noConfig" | "completed";
  /** Names of detected agents without findings (for `noConfig`). */
  emptyAgents: string[];
  /** At least one diagnostic with level `error`. */
  partial: boolean;
  unreadable: Diagnostic[];
  invalid: Diagnostic[];
}

export function reportState(report: ScanReport): ReportState {
  const detected = report.results.filter((r) => r.detection.status !== "notDetected");
  const diagnostics = allDiagnostics(report);
  const emptyAgents = detected.filter((r) => r.findings.length === 0).map((r) => r.scanner.name);
  let kind: ReportState["kind"] = "completed";
  if (detected.length === 0) {
    kind = "noAgent";
  } else if (emptyAgents.length === detected.length) {
    kind = "noConfig";
  }
  return {
    kind,
    emptyAgents,
    partial: diagnostics.some((d) => d.level === "error"),
    unreadable: diagnostics.filter((d) => UNREADABLE_CODES.includes(d.code)),
    invalid: diagnostics.filter((d) => INVALID_CODES.includes(d.code)),
  };
}
