import { useI18n } from "../i18n";
import type { ReportState } from "../report";

/** Presentations for the scan states of roadmap section 12.4. */
export function StateBanner({ state }: { state: ReportState }) {
  const { t } = useI18n();
  const messages: { tone: "info" | "warning" | "error"; text: string }[] = [];
  if (state.kind === "noAgent") {
    messages.push({ tone: "info", text: t("state.noAgent") });
  } else if (state.kind === "noConfig") {
    messages.push({ tone: "info", text: t("state.noConfig", { agent: state.emptyAgents.join(", ") }) });
  }
  if (state.partial) {
    messages.push({ tone: "error", text: t("state.partial") });
  }
  if (state.unreadable.length > 0) {
    messages.push({ tone: "warning", text: t("state.unreadable", { count: state.unreadable.length }) });
  }
  if (state.invalid.length > 0) {
    messages.push({ tone: "warning", text: t("state.invalid", { count: state.invalid.length }) });
  }
  if (messages.length === 0) {
    messages.push({ tone: "info", text: t("state.completed") });
  }
  return (
    <div role="status">
      {messages.map((message) => (
        <p key={message.text} className={`banner banner-${message.tone}`}>
          {message.text}
        </p>
      ))}
    </div>
  );
}
