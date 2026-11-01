import { useI18n } from "../i18n";
import { EMPTY_FILTERS, FILTER_FIELDS, fieldOptions } from "../filters";
import type { FilterField, Filters } from "../filters";
import type { Finding } from "../types/report";

interface Props {
  findings: Finding[];
  filters: Filters;
  onChange: (filters: Filters) => void;
}

const LABEL_KEYS: Record<FilterField, string> = {
  agent: "filter.agent",
  status: "column.status",
  effect: "column.effect",
  access: "column.access",
  scope: "column.scope",
};

export function FilterBar({ findings, filters, onChange }: Props) {
  const { t } = useI18n();
  const agentNames = new Map(findings.map((f) => [f.agentId, f.agentName]));
  const optionLabel = (field: FilterField, value: string) =>
    field === "agent" ? (agentNames.get(value) ?? value) : t(`${field}.${value}`);

  return (
    <fieldset className="filters">
      <legend>{t("filter.heading")}</legend>
      {FILTER_FIELDS.map((field) => (
        <label key={field}>
          {t(LABEL_KEYS[field])}
          <select value={filters[field]} onChange={(e) => onChange({ ...filters, [field]: e.target.value })}>
            <option value="">{t("filter.all")}</option>
            {fieldOptions(findings, field).map((value) => (
              <option key={value} value={value}>
                {optionLabel(field, value)}
              </option>
            ))}
          </select>
        </label>
      ))}
      <label className="check">
        <input
          type="checkbox"
          checked={filters.warningsOnly}
          onChange={(e) => onChange({ ...filters, warningsOnly: e.target.checked })}
        />
        {t("filter.warningsOnly")}
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={filters.existingOnly}
          onChange={(e) => onChange({ ...filters, existingOnly: e.target.checked })}
        />
        {t("filter.existingOnly")}
      </label>
      <button type="button" className="secondary" onClick={() => onChange(EMPTY_FILTERS)}>
        {t("filter.reset")}
      </button>
    </fieldset>
  );
}
