import { useRef, useState } from "react";
import { api } from "../api";
import { CATALOGS, useI18n } from "../i18n";
import type { ExportFormat, PathPrivacy } from "../types/report";
import { errorText } from "./errors";

const FORMATS: ExportFormat[] = ["markdown", "json"];
const PRIVACY: PathPrivacy[] = ["full", "home", "anonymized"];

/** Export dialog (roadmap section 13). The backend asks where to save. */
export function ExportDialog() {
  const { t, language } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const [format, setFormat] = useState<ExportFormat>("markdown");
  const [privacy, setPrivacy] = useState<PathPrivacy>("full");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  const open = () => {
    setStatus("");
    dialog.current?.showModal();
  };

  const submit = () => {
    setBusy(true);
    // Markdown uses the GUI language; JSON always carries English texts.
    const translations = format === "markdown" && language !== "en" ? CATALOGS[language] : null;
    api
      .exportScan(format, privacy, translations)
      .then(
        (path) => setStatus(path ? t("export.saved", { path }) : t("export.cancelled")),
        (error: unknown) => setStatus(errorText(error, t)),
      )
      .finally(() => setBusy(false));
  };

  return (
    <>
      <button type="button" className="secondary" onClick={open}>
        {t("action.export")}
      </button>
      <dialog ref={dialog} aria-labelledby="export-heading" className="export-dialog">
        <h2 id="export-heading">{t("export.heading")}</h2>
        <fieldset>
          <legend>{t("export.format")}</legend>
          {FORMATS.map((value) => (
            <label key={value}>
              <input
                type="radio"
                name="export-format"
                value={value}
                checked={format === value}
                onChange={() => setFormat(value)}
              />
              {t(`export.format.${value}`)}
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>{t("export.privacy")}</legend>
          {PRIVACY.map((value) => (
            <label key={value}>
              <input
                type="radio"
                name="export-privacy"
                value={value}
                checked={privacy === value}
                onChange={() => setPrivacy(value)}
              />
              {t(`export.privacy.${value}`)}
            </label>
          ))}
        </fieldset>
        <p className="small">{t("export.note")}</p>
        <p className="small" role="status" aria-live="polite">
          {status}
        </p>
        <div className="actions">
          <button type="button" onClick={submit} disabled={busy}>
            {t("export.submit")}
          </button>
          <button type="button" className="secondary" onClick={() => dialog.current?.close()}>
            {t("action.close")}
          </button>
        </div>
      </dialog>
    </>
  );
}
