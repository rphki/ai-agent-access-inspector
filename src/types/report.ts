// Types of the scan report as serialised by `inspector-core` and the Tauri
// commands in `src-tauri/src/commands`. See docs/findings-model.md.

export interface Message {
  key: string;
  args: Record<string, string>;
  /** English text rendered by the core; fallback for unknown keys. */
  text: string;
}

export type RuleEffect = "allow" | "ask" | "deny" | "notApplicable";
export type AccessKind = "read" | "write" | "execute" | "readWrite" | "unknown";
export type FindingStatus = "configured" | "observed" | "potential" | "blocked" | "unknown";
export type FindingScope =
  | "managed"
  | "user"
  | "project"
  | "projectLocal"
  | "session"
  | "runtime"
  | "operatingSystem";
export type Confidence = "low" | "medium" | "high";
export type PatternKind = "exactPath" | "directory" | "glob" | "everything" | "unparsed";
export type SourceDocumentation = "documented" | "heuristic" | "derived";
export type Existence = "exists" | "missing" | "unknown" | "notChecked";
export type PathKind = "file" | "directory" | "other" | "unknown";
export type Severity = "info" | "low" | "medium" | "high";
export type Relation = "contains" | "containedIn" | "overriddenBy";
export type DiagnosticLevel = "info" | "warning" | "error";

export type FindingSubject = { kind: "agent" } | { kind: "mcpServer"; name: string };

export type PathAnchor =
  | { kind: "filesystemRoot" }
  | { kind: "home" }
  | { kind: "workingDirectory" }
  | { kind: "directory"; path: string };

export interface FindingTarget {
  raw: string;
  patternKind: PatternKind;
  basePath: string | null;
  displayPath: string;
}

export interface FindingSource {
  file: string | null;
  configKind: string;
  documentation: SourceDocumentation;
  pointer: string | null;
  rawValue: string | null;
  anchor: PathAnchor | null;
  parserVersion: string;
  excerpt: string | null;
}

export interface PathState {
  existence: Existence;
  kind: PathKind;
  symlinkTarget: string | null;
}

export interface Warning {
  kind: { type: string; reason?: string };
  severity: Severity;
  message: Message;
}

export interface RelatedFinding {
  relation: Relation;
  findingId: string;
}

export interface Finding {
  id: string;
  agentId: string;
  agentName: string;
  subject: FindingSubject;
  target: FindingTarget;
  effect: RuleEffect;
  access: AccessKind;
  status: FindingStatus;
  scope: FindingScope;
  source: FindingSource;
  additionalSources: FindingSource[];
  explanation: Message;
  confidence: Confidence;
  pathState: PathState;
  warnings: Warning[];
  related: RelatedFinding[];
}

export interface Diagnostic {
  level: DiagnosticLevel;
  code: string;
  message: Message;
  file: string | null;
  pointer: string | null;
}

export interface SummaryEntry {
  key: string;
  label: Message;
  value: Message;
}

export interface ScannerMetadata {
  id: string;
  name: string;
  description: string;
  parserVersion: string;
}

export interface DetectionResult {
  status: "notDetected" | "configured";
  evidence: string[];
  version: string | null;
}

export interface ScanResult {
  scanner: ScannerMetadata;
  detection: DetectionResult;
  findings: Finding[];
  diagnostics: Diagnostic[];
  summary: SummaryEntry[];
}

export interface ScanReport {
  formatVersion: string;
  tool: { name: string; version: string };
  operatingSystem: "macos" | "linux" | "windows";
  results: ScanResult[];
}

/** Result of `run_scan` and `get_scan_details`. */
export interface StoredScan {
  report: ScanReport;
  /** Milliseconds since the Unix epoch. */
  finishedAt: number;
}

export interface AppInfo {
  name: string;
  version: string;
  operatingSystem: ScanReport["operatingSystem"];
}

export type CommandErrorKind =
  | "scanContext"
  | "scanFailed"
  | "noScan"
  | "pathNotInReport"
  | "openFailed"
  | "exportFailed";

export type ExportFormat = "json" | "markdown";
export type PathPrivacy = "full" | "home" | "anonymized";

export interface CommandError {
  kind: CommandErrorKind;
  detail: string | null;
}
