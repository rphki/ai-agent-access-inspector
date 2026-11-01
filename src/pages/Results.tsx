import { useMemo, useState } from "react";
import { FilterBar } from "../components/FilterBar";
import { subjectLabel } from "../components/labels";
import { EMPTY_FILTERS, filterFindings } from "../filters";
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
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const shown = useMemo(() => filterFindings(findings, filters), [findings, filters]);

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
                <th scope="col">{t("column.access")}</th>
                <th scope="col">{t("column.scope")}</th>
                <th scope="col">{t("column.confidence")}</th>
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
                      <button
                        type="button"
                        className="link mono"
                        aria-label={t("results.select", { path: finding.target.displayPath })}
                        onClick={() => onSelect(finding.id)}
                      >
                        {finding.target.displayPath}
                      </button>
                    </td>
                    <td>{t(`effect.${finding.effect}`)}</td>
                    <td>{t(`status.${finding.status}`)}</td>
                    <td>{t(`access.${finding.access}`)}</td>
                    <td>{t(`scope.${finding.scope}`)}</td>
                    <td>{t(`confidence.${finding.confidence}`)}</td>
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
