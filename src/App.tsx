import { Component, useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { api } from "./api";
import { errorText } from "./components/errors";
import { LANGUAGES, useI18n } from "./i18n";
import type { Language } from "./i18n";
import { Detail } from "./pages/Detail";
import { Overview } from "./pages/Overview";
import { Results } from "./pages/Results";
import { allFindings } from "./report";
import type { StoredScan } from "./types/report";

export default function App() {
  const { t, language, setLanguage } = useI18n();
  const [scan, setScan] = useState<StoredScan | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const runScan = useCallback(() => {
    setRunning(true);
    setError(null);
    api
      .runScan()
      .then(setScan, setError)
      .finally(() => setRunning(false));
  }, []);

  useEffect(() => {
    // Reuse the backend's last scan after a reload; scan on first start.
    api.getScanDetails().then(
      (stored) => (stored ? setScan(stored) : runScan()),
      () => runScan(),
    );
  }, [runScan]);

  const findings = useMemo(() => (scan ? allFindings(scan.report) : []), [scan]);
  const selected = findings.find((f) => f.id === selectedId) ?? null;

  return (
    <div className="app">
      <header>
        <div>
          <h1>{t("app.title")}</h1>
          <p className="small">
            {t("app.tagline")}
          </p>
        </div>
        <label>
          {t("app.language")}{" "}
          <select value={language} onChange={(e) => setLanguage(e.target.value as Language)}>
            {LANGUAGES.map((lang) => (
              <option key={lang} value={lang}>
                {t(`language.${lang}`)}
              </option>
            ))}
          </select>
        </label>
      </header>
      <main>
        {error !== null && (
          <div role="alert" className="banner banner-error">
            <p>
              {t("state.internalError")} {errorText(error, t)}
            </p>
            <button type="button" onClick={runScan} disabled={running}>
              {t("action.scanAgain")}
            </button>
          </div>
        )}
        {!scan && running && <p role="status">{t("state.running")}</p>}
        {!scan && !running && error === null && <p role="status">{t("state.loading")}</p>}
        {scan && (
          <>
            <Overview scan={scan} running={running} onScan={runScan} />
            {findings.length > 0 && (
              <div className="split">
                <Results findings={findings} selectedId={selectedId} onSelect={setSelectedId} />
                <Detail key={selected?.id ?? ""} finding={selected} findings={findings} onSelect={setSelectedId} />
              </div>
            )}
          </>
        )}
      </main>
    </div>
  );
}

/** Shows the "unexpected internal error" state instead of a blank window. */
export class ErrorBoundary extends Component<{ fallback: ReactNode; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}
