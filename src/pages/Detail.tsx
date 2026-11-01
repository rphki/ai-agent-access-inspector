import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { errorText } from "../components/errors";
import { subjectLabel } from "../components/labels";
import { useI18n } from "../i18n";
import type { Finding, FindingSource, PathAnchor } from "../types/report";

interface Props {
  finding: Finding | null;
  findings: Finding[];
  onSelect: (id: string) => void;
}

/** Detail view (roadmap section 12.3). */
export function Detail({ finding, findings, onSelect }: Props) {
  const { t, tm } = useI18n();
  const heading = useRef<HTMLHeadingElement>(null);
  const [status, setStatus] = useState("");

  // App remounts this component per finding (`key`), which also clears `status`.
  useEffect(() => {
    if (finding) {
      heading.current?.focus();
    }
  }, [finding]);

  if (!finding) {
    return (
      <section aria-labelledby="detail-heading" className="detail">
        <h2 id="detail-heading">{t("detail.heading")}</h2>
        <p>{t("detail.noSelection")}</p>
      </section>
    );
  }

  const none = t("detail.none");
  const run = (action: () => Promise<unknown>, success = "") =>
    action().then(
      () => setStatus(success),
      (error: unknown) => setStatus(errorText(error, t)),
    );
  const copy = (text: string) =>
    navigator.clipboard.writeText(text).then(
      () => setStatus(t("action.copied")),
      () => setStatus(t("action.copyFailed")),
    );
  const anchorText = (anchor: PathAnchor | null) =>
    anchor ? t(`anchor.${anchor.kind}`, anchor.kind === "directory" ? { path: anchor.path } : {}) : null;
  const byId = new Map(findings.map((f) => [f.id, f]));
  const basePath = finding.target.basePath;
  const sourceFile = finding.source.file;

  const source = (src: FindingSource) => (
    <dl className="fields">
      <Field label={t("detail.file")} mono value={src.file ?? none} />
      <Field label={t("detail.configKind")} mono value={src.configKind} />
      <Field label={t("detail.documentation")} value={t(`documentation.${src.documentation}`)} />
      <Field label={t("detail.pointer")} mono value={src.pointer ?? none} />
      <Field label={t("detail.rawValue")} mono value={src.rawValue ?? none} />
      {src.anchor && <Field label={t("detail.anchor")} value={anchorText(src.anchor)} />}
      {src.excerpt && <Field label={t("detail.excerpt")} mono value={src.excerpt} />}
      <Field label={t("detail.parserVersion")} value={src.parserVersion} />
    </dl>
  );

  return (
    <section aria-labelledby="detail-heading" className="detail">
      <h2 id="detail-heading" ref={heading} tabIndex={-1}>
        {t("detail.heading")}: <span className="mono">{finding.target.displayPath}</span>
      </h2>

      <div className="actions">
        <button type="button" disabled={!basePath} onClick={() => basePath && run(() => api.openPathInFileManager(basePath))}>
          {t("action.openFolder")}
        </button>
        <button type="button" disabled={!sourceFile} onClick={() => sourceFile && run(() => api.openSourceFile(sourceFile))}>
          {t("action.openConfig")}
        </button>
        <button type="button" className="secondary" onClick={() => copy(basePath ?? finding.target.displayPath)}>
          {t("action.copyPath")}
        </button>
        <button type="button" className="secondary" onClick={() => copy(JSON.stringify(finding, null, 2))}>
          {t("action.copyJson")}
        </button>
      </div>
      <p className="small" role="status" aria-live="polite">
        {status}
      </p>

      <p className="explanation">{tm(finding.explanation)}</p>

      <dl className="fields">
        <Field label={t("column.subject")} value={subjectLabel(finding, t)} />
        <Field label={t("detail.path")} mono value={basePath ?? none} />
        <Field label={t("detail.raw")} mono value={finding.target.raw} />
        <Field label={t("detail.patternKind")} value={t(`patternKind.${finding.target.patternKind}`)} />
        <Field label={t("column.effect")} value={t(`effect.${finding.effect}`)} />
        <Field label={t("column.status")} value={t(`status.${finding.status}`)} />
        <Field label={t("column.access")} value={t(`access.${finding.access}`)} />
        <Field label={t("column.scope")} value={t(`scope.${finding.scope}`)} />
        <Field label={t("column.confidence")} value={t(`confidence.${finding.confidence}`)} />
      </dl>

      <h3>{t("detail.pathState")}</h3>
      <dl className="fields">
        <Field label={t("detail.existence")} value={t(`existence.${finding.pathState.existence}`)} />
        <Field label={t("detail.pathKind")} value={t(`pathKind.${finding.pathState.kind}`)} />
        <Field
          label={t("detail.symlink")}
          mono={!!finding.pathState.symlinkTarget}
          value={finding.pathState.symlinkTarget ?? t("detail.noSymlink")}
        />
      </dl>

      <h3>{t("detail.warnings")}</h3>
      {finding.warnings.length === 0 ? (
        <p>{t("detail.noWarnings")}</p>
      ) : (
        <ul className="warnings">
          {finding.warnings.map((warning, index) => (
            <li key={index}>
              <span className={`badge severity-${warning.severity}`}>{t(`severity.${warning.severity}`)}</span>{" "}
              {tm(warning.message)}
            </li>
          ))}
        </ul>
      )}

      {finding.related.length > 0 && (
        <>
          <h3>{t("detail.related")}</h3>
          <ul className="related">
            {finding.related.map((related) => {
              const other = byId.get(related.findingId);
              return (
                <li key={`${related.relation}-${related.findingId}`}>
                  {t(`relation.${related.relation}`)}:{" "}
                  {other ? (
                    <button type="button" className="link mono" onClick={() => onSelect(other.id)}>
                      {other.target.displayPath}
                    </button>
                  ) : (
                    <span className="mono">{related.findingId}</span>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}

      <h3>{t("detail.source")}</h3>
      {source(finding.source)}
      {finding.additionalSources.length > 0 && (
        <>
          <h3>{t("detail.additionalSources")}</h3>
          {finding.additionalSources.map((src, index) => (
            <div key={index}>{source(src)}</div>
          ))}
        </>
      )}
    </section>
  );
}

function Field({ label, value, mono }: { label: string; value: ReactNode; mono?: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={mono ? "mono" : undefined}>{value}</dd>
    </div>
  );
}
