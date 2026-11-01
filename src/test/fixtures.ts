// Scan reports for tests, built from the reviewed scanner fixtures.
import linuxBasic from "../../fixtures/claude-code/linux-basic/expected.json";
import malformed from "../../fixtures/claude-code/malformed/expected.json";
import notDetected from "../../fixtures/claude-code/not-detected/expected.json";
import type { ScanResult, StoredScan } from "../types/report";

export function storedScan(...results: unknown[]): StoredScan {
  return {
    report: {
      formatVersion: "0.1",
      tool: { name: "ai-agent-access-inspector", version: "0.0.0" },
      operatingSystem: "linux",
      results: results as ScanResult[],
    },
    finishedAt: Date.UTC(2026, 10, 1, 12, 0, 0),
  };
}

export const LINUX_BASIC = storedScan(linuxBasic);
export const MALFORMED = storedScan(malformed);
export const NOT_DETECTED = storedScan(notDetected);
/** Detected, but without findings. */
export const NO_CONFIG = storedScan({ ...notDetected, detection: { ...notDetected.detection, status: "configured" } });
