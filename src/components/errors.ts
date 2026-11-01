import { isCommandError } from "../api";

type T = (key: string, args?: Record<string, string | number>) => string;

/** Text for an error returned by a Tauri command, with its technical detail. */
export function errorText(error: unknown, t: T): string {
  if (!isCommandError(error)) {
    return `${t("error.unknown")} ${t("error.detail", { detail: String(error) })}`;
  }
  const text = t(`error.${error.kind}`);
  return error.detail ? `${text} ${t("error.detail", { detail: error.detail })}` : text;
}
