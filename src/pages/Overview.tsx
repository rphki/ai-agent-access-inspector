import { Diagnostics } from "../components/Diagnostics";
import { ExportDialog } from "../components/ExportDialog";
import { StateBanner } from "../components/StateBanner";
import { useI18n } from "../i18n";
import { allDiagnostics, allFindings, reportState, warningCount } from "../report";
import type { StoredScan } from "../types/report";

interface Props {
  scan: StoredScan;
  running: boolean;
  onScan: () => void;
}

/** Main view (roadmap section 12.1). */
export function Overview({ scan, running, onScan }: Props) {
  const { t, tm, language } = useI18n();
  const { report } = scan;
  const lastScan = new Intl.DateTimeFormat(language, { dateStyle: "medium", timeStyle: "medium" }).format(
    new Date(scan.finishedAt),
  );

  return (
    <section aria-label={t("overview.heading")} className="overview">
      <div className="overview-head">
        <dl className="stats">
          <div>
            <dt>{t("overview.lastScan")}</dt>
            <dd>{lastScan}</dd>
          </div>
          <div>
            <dt>{t("overview.findings")}</dt>
            <dd>{allFindings(report).length}</dd>
          </div>
          <div>
            <dt>{t("overview.warnings")}</dt>
            <dd>{warningCount(report)}</dd>
          </div>
        </dl>
        <div className="actions">
          <ExportDialog />
          <button type="button" onClick={onScan} disabled={running}>
            {running ? t("state.running") : t("action.scanAgain")}
          </button>
        </div>
      </div>
      <StateBanner state={reportState(report)} />
      <h3>{t("overview.agents")}</h3>
      <ul className="agents">
        {report.results.map((result) => (
          <li key={result.scanner.id}>
            <details>
              <summary>
                <strong>{result.scanner.name}</strong>{" "}
                <span className={`badge detection-${result.detection.status}`}>
                  {t(`detection.${result.detection.status}`)}
                </span>
                {result.detection.version && <> · {t("overview.version", { version: result.detection.version })}</>}
              </summary>
              {result.detection.evidence.length > 0 && (
                <div className="small">
                  {t("overview.evidence")}:{" "}
                  <span className="mono">{result.detection.evidence.join(", ")}</span>
                </div>
              )}
              {result.summary.length > 0 && (
                <dl className="summary" aria-label={t("overview.summary")}>
                  {result.summary.map((entry) => (
                    <div key={entry.key}>
                      <dt>{tm(entry.label)}</dt>
                      <dd>{tm(entry.value)}</dd>
                    </div>
                  ))}
                </dl>
              )}
            </details>
          </li>
        ))}
      </ul>
      <Diagnostics diagnostics={allDiagnostics(report)} />
    </section>
  );
}
