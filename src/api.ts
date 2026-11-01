// The only place that talks to the Tauri backend.
import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, CommandError, StoredScan } from "./types/report";

export const api = {
  runScan: () => invoke<StoredScan>("run_scan"),
  getScanDetails: () => invoke<StoredScan | null>("get_scan_details"),
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  openPathInFileManager: (path: string) => invoke<void>("open_path_in_file_manager", { path }),
  openSourceFile: (path: string) => invoke<void>("open_source_file", { path }),
};

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "kind" in value;
}
