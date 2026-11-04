import { create } from "zustand";
import en, { type TranslationKeys } from "./en";
import es from "./es";

export type Locale = "en" | "es";

const translations: Record<Locale, Record<string, string>> = { en, es };

interface I18nState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useI18nStore = create<I18nState>((set) => ({
  locale: (navigator.language.startsWith("es") ? "es" : "en") as Locale,
  setLocale: (locale: Locale) => set({ locale }),
}));

/** Texto suelto en el idioma actual, para usar fuera de los componentes (sin parámetros). */
export function translate(key: TranslationKeys): string {
  const dict = translations[useI18nStore.getState().locale];
  return dict[key] || translations.en[key] || key;
}

export function useTranslation() {
  const locale = useI18nStore((s) => s.locale);
  const dict = translations[locale];

  function t(key: TranslationKeys, params?: Record<string, string | number>): string {
    let text = dict[key] || translations.en[key] || key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        text = text.replace(`{${k}}`, String(v));
      }
    }
    return text;
  }

  return { t, locale };
}
