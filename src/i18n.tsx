// Translation of the GUI (German, English). Core texts arrive as messages
// (key, args, English text); `src/locales/en.json` is generated from the
// core catalog, the other files are maintained by hand. Parameters such as
// paths are inserted as they are and never translated.
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import coreDe from "./locales/de.json";
import coreEn from "./locales/en.json";
import uiDe from "./locales/ui.de.json";
import uiEn from "./locales/ui.en.json";
import type { Message } from "./types/report";

export type Language = "en" | "de";
export const LANGUAGES: Language[] = ["en", "de"];

type Catalog = Record<string, string>;
export const CATALOGS: Record<Language, Catalog> = {
  en: { ...coreEn, ...uiEn },
  de: { ...coreDe, ...uiDe },
};

const STORAGE_KEY = "language";

export function fill(template: string, args: Record<string, string | number> = {}): string {
  return template.replace(/\{([A-Za-z0-9_]+)\}/g, (match, name: string) =>
    name in args ? String(args[name]) : match,
  );
}

/** Translates a key; falls back to English, then to `fallback`, then to the key. */
export function translate(
  language: Language,
  key: string,
  args?: Record<string, string | number>,
  fallback?: string,
): string {
  const template = CATALOGS[language][key] ?? CATALOGS.en[key];
  if (template === undefined) {
    return fallback ?? key;
  }
  return fill(template, args);
}

export function translateMessage(language: Language, message: Message): string {
  return translate(language, message.key, message.args, message.text);
}

export function initialLanguage(): Language {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "en" || stored === "de") {
      return stored;
    }
  } catch {
    // Storage can be unavailable; the system language is used instead.
  }
  return navigator.language.toLowerCase().startsWith("de") ? "de" : "en";
}

interface I18n {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: string, args?: Record<string, string | number>) => string;
  tm: (message: Message) => string;
}

const I18nContext = createContext<I18n | null>(null);

export function I18nProvider({ children, language: forced }: { children: ReactNode; language?: Language }) {
  const [language, setLanguageState] = useState<Language>(() => forced ?? initialLanguage());

  const setLanguage = useCallback((next: Language) => {
    setLanguageState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Not remembered; the choice still applies until the app is closed.
    }
  }, []);

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const value = useMemo<I18n>(
    () => ({
      language,
      setLanguage,
      t: (key, args) => translate(language, `ui.${key}`, args),
      tm: (message) => translateMessage(language, message),
    }),
    [language, setLanguage],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return context;
}
