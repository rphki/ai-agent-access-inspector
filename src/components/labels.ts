import type { Finding } from "../types/report";

type T = (key: string, args?: Record<string, string | number>) => string;

/** "Agent or MCP server" column of a finding. */
export function subjectLabel(finding: Finding, t: T): string {
  return finding.subject.kind === "mcpServer"
    ? t("detail.subjectMcp", { name: finding.subject.name })
    : finding.agentName;
}
