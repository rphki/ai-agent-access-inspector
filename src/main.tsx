import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App, { ErrorBoundary } from "./App";
import { I18nProvider, initialLanguage, translate } from "./i18n";
import "./App.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary fallback={<p role="alert">{translate(initialLanguage(), "ui.state.internalError")}</p>}>
      <I18nProvider>
        <App />
      </I18nProvider>
    </ErrorBoundary>
  </StrictMode>,
);
