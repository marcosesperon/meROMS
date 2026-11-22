import { create } from "zustand";
import en, { type TranslationKeys } from "./en";
import es from "./es";
import fr from "./fr";
import pt from "./pt";
import it from "./it";
import de from "./de";

/** Idiomas de la app, en el orden del selector de Ajustes */
export const LOCALES = ["en", "es", "fr", "pt", "it", "de"] as const;
export type Locale = (typeof LOCALES)[number];

/** Nombre de cada idioma en su propio idioma (selector de Ajustes) */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  es: "Español",
  fr: "Français",
  pt: "Português",
  it: "Italiano",
  de: "Deutsch",
};

export const isLocale = (value: unknown): value is Locale => LOCALES.includes(value as Locale);

const translations: Record<Locale, Record<string, string>> = { en, es, fr, pt, it, de };

/** El idioma del sistema operativo si es uno de los de la app; si no, inglés */
function systemLocale(): Locale {
  const lang = navigator.language.slice(0, 2).toLowerCase();
  return isLocale(lang) ? lang : "en";
}

interface I18nState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useI18nStore = create<I18nState>((set) => ({
  locale: systemLocale(),
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
