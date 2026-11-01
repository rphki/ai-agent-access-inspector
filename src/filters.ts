// Filtering of the results list (roadmap section 12.2).
import type { Finding } from "./types/report";

export const FILTER_FIELDS = ["agent", "status", "effect", "access", "scope"] as const;
export type FilterField = (typeof FILTER_FIELDS)[number];

export interface Filters {
  agent: string;
  status: string;
  effect: string;
  access: string;
  scope: string;
  warningsOnly: boolean;
  existingOnly: boolean;
}

export const EMPTY_FILTERS: Filters = {
  agent: "",
  status: "",
  effect: "",
  access: "",
  scope: "",
  warningsOnly: false,
  existingOnly: false,
};

/** The value a finding has for a filter field; `agent` is the agent id. */
export function fieldValue(finding: Finding, field: FilterField): string {
  return field === "agent" ? finding.agentId : finding[field];
}

export function filterFindings(findings: Finding[], filters: Filters): Finding[] {
  return findings.filter(
    (finding) =>
      FILTER_FIELDS.every((field) => !filters[field] || fieldValue(finding, field) === filters[field]) &&
      (!filters.warningsOnly || finding.warnings.length > 0) &&
      (!filters.existingOnly || finding.pathState.existence === "exists"),
  );
}

/** Distinct values of a field in the given findings, in first-seen order. */
export function fieldOptions(findings: Finding[], field: FilterField): string[] {
  return [...new Set(findings.map((finding) => fieldValue(finding, field)))];
}
