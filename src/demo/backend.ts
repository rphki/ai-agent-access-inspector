// Mock of `@tauri-apps/api/core` for `pnpm dev:demo` (see vite.config.ts):
// runs the GUI in a browser with a fixture report, e.g. for screenshots.
// Not part of the application build.
import mcp from "../../fixtures/claude-code/mcp/expected.json";
import codex from "../../fixtures/codex/mcp/expected.json";
import type { StoredScan } from "../types/report";

const scan = {
  report: {
    formatVersion: "0.1",
    tool: { name: "ai-agent-access-inspector", version: "demo" },
    operatingSystem: "linux",
    results: [mcp, codex],
  },
  finishedAt: Date.UTC(2026, 10, 1, 9, 30),
} as unknown as StoredScan;

export async function invoke<T>(command: string): Promise<T> {
  const responses: Record<string, unknown> = {
    get_app_info: { name: "ai-agent-access-inspector", version: "demo", operatingSystem: "linux" },
    get_scan_details: scan,
    run_scan: scan,
    export_scan: "/home/alice/ai-agent-access-report-2026-11-01.md",
  };
  return responses[command] as T;
}
