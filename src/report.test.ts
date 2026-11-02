import { EMPTY_FILTERS, fieldOptions, filterFindings } from "./filters";
import { allFindings, maxSeverity, reportState } from "./report";
import { LINUX_BASIC, MALFORMED, NO_CONFIG, NOT_DETECTED } from "./test/fixtures";

describe("reportState", () => {
  it("recognises the scan states", () => {
    expect(reportState(NOT_DETECTED.report).kind).toBe("noAgent");
    expect(reportState(NO_CONFIG.report)).toMatchObject({ kind: "noConfig", emptyAgents: ["Claude Code"] });
    expect(reportState(LINUX_BASIC.report)).toMatchObject({ kind: "completed", partial: false });
  });

  it("reports unreadable and invalid configuration as partial", () => {
    const state = reportState(MALFORMED.report);
    expect(state.partial).toBe(true);
    expect(state.unreadable).toHaveLength(1);
    expect(state.invalid.length).toBeGreaterThan(1);
  });

  it("finds the highest warning severity", () => {
    const warnings = allFindings(MALFORMED.report).flatMap((f) => f.warnings);
    expect(maxSeverity([])).toBeNull();
    expect(maxSeverity(warnings)).not.toBeNull();
  });
});

describe("filterFindings", () => {
  const findings = allFindings(LINUX_BASIC.report);

  it("keeps everything without filters", () => {
    expect(filterFindings(findings, EMPTY_FILTERS)).toHaveLength(findings.length);
  });

  it("combines field and checkbox filters", () => {
    const denies = filterFindings(findings, { ...EMPTY_FILTERS, effect: "deny" });
    expect(denies.map((f) => f.target.displayPath)).toContain("/etc/shadow");
    const existingDenies = filterFindings(findings, { ...EMPTY_FILTERS, effect: "deny", hideMissing: true });
    expect(existingDenies.map((f) => f.target.displayPath)).not.toContain("/etc/shadow");
    expect(filterFindings(findings, { ...EMPTY_FILTERS, warningsOnly: true })).toHaveLength(1);
  });

  it("offers only values that occur", () => {
    expect(fieldOptions(findings, "agent")).toEqual(["claude-code"]);
    expect(fieldOptions(findings, "effect")).not.toContain("ask");
  });
});
