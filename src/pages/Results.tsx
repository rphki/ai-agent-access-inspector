import { useMemo, useState } from "react";
import { FilterBar } from "../components/FilterBar";
import { subjectLabel } from "../components/labels";
import { DEFAULT_FILTERS, filterFindings } from "../filters";
import { useI18n } from "../i18n";
import { maxSeverity } from "../report";
import type { Finding } from "../types/report";

interface Props {
  findings: Finding[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/** Results view (roadmap section 12.2). */
export function Results({ findings, selectedId, onSelect }: Props) {
  const { t } = useI18n();
  const [filters, setFilters] = useState(DEFAULT_FILTERS);
  const shown = useMemo(() => {
    const filtered = filterFindings(findings, filters);
    // Missing paths last; the sort is stable, so the order is kept otherwise.
    return filtered.sort((a, b) => Number(isMissing(a)) - Number(isMissing(b)));
  }, [findings, filters]);

  return (
    <section aria-labelledby="results-heading" className="results">
      <h2 id="results-heading">{t("results.heading")}</h2>
      <FilterBar findings={findings} filters={filters} onChange={setFilters} />
      <p className="small" aria-live="polite">
        {t("results.count", { shown: shown.length, total: findings.length })}
      </p>
      {shown.length === 0 ? (
        <p>{t("results.empty")}</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th scope="col">{t("column.subject")}</th>
                <th scope="col">{t("column.target")}</th>
                <th scope="col">{t("column.effect")}</th>
                <th scope="col">{t("column.status")}</th>
                <th scope="col">{t("column.scope")}</th>
                <th scope="col">{t("column.warnings")}</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((finding) => {
                const severity = maxSeverity(finding.warnings);
                return (
                  <tr key={finding.id} aria-selected={finding.id === selectedId}>
                    <td>{subjectLabel(finding, t)}</td>
                    <td>
                      {isMissing(finding) ? (
                        <span className="mono missing" title={t("results.missing")}>
                          <GhostIcon label={t("results.missing")} />
                          {finding.target.displayPath}
                        </span>
                      ) : (
                        <button
                          type="button"
                          className="link mono"
                          aria-label={t("results.select", { path: finding.target.displayPath })}
                          onClick={() => onSelect(finding.id)}
                        >
                          {finding.target.displayPath}
                        </button>
                      )}
                    </td>
                    <td>{t(`effect.${finding.effect}`)}</td>
                    <td>{t(`status.${finding.status}`)}</td>
                    <td>{t(`scope.${finding.scope}`)}</td>
                    <td>
                      {severity && (
                        <span className={`badge severity-${severity}`}>
                          {finding.warnings.length} · {t(`severity.${severity}`)}
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function isMissing(finding: Finding): boolean {
  return finding.pathState.existence === "missing";
}

function GhostIcon({ label }: { label: string }) {
  return (
    <svg className="ghost" viewBox="0 0 16 16" width="14" height="14" role="img" aria-label={label}>
      <path
        fill="currentColor"
        d="M8 1a5.5 5.5 0 0 0-5.5 5.5V15l2-1.5L6.25 15 8 13.5 9.75 15l1.75-1.5L13.5 15V6.5A5.5 5.5 0 0 0 8 1Zm-2 4.5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Zm4 0a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z"
      />
    </svg>
  );
}
