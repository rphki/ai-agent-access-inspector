import type { Finding } from "../types/report";

type T = (key: string, args?: Record<string, string | number>) => string;

/**
 * "Agent or MCP server" column of a finding. MCP servers carry the agent's
 * name too, since several agents can have a server of the same name.
 */
export function subjectLabel(finding: Finding, t: T): string {
  return finding.subject.kind === "mcpServer"
    ? `${finding.agentName} · ${t("detail.subjectMcp", { name: finding.subject.name })}`
    : finding.agentName;
}
