import { useI18n } from "../i18n";
import type { Diagnostic } from "../types/report";

export function Diagnostics({ diagnostics }: { diagnostics: Diagnostic[] }) {
  const { t, tm } = useI18n();
  if (diagnostics.length === 0) {
    return null;
  }
  return (
    <details className="diagnostics" open={diagnostics.some((d) => d.level === "error")}>
      <summary>
        {t("overview.diagnostics")} ({diagnostics.length})
      </summary>
      <ul>
        {diagnostics.map((diagnostic, index) => (
          <li key={index} className={`level-${diagnostic.level}`}>
            <span className="badge">{t(`level.${diagnostic.level}`)}</span> {tm(diagnostic.message)}
            {diagnostic.file && (
              <div className="mono small">
                {diagnostic.file}
                {diagnostic.pointer && ` · ${diagnostic.pointer}`}
              </div>
            )}
          </li>
        ))}
      </ul>
    </details>
  );
}
