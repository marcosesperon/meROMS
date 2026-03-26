/** Map common language/region codes to flag emoji. */
const FLAG_MAP: Record<string, string> = {
  // Languages
  en: "🇬🇧",
  es: "🇪🇸",
  fr: "🇫🇷",
  de: "🇩🇪",
  it: "🇮🇹",
  pt: "🇵🇹",
  ja: "🇯🇵",
  zh: "🇨🇳",
  ko: "🇰🇷",
  nl: "🇳🇱",
  sv: "🇸🇪",
  da: "🇩🇰",
  no: "🇳🇴",
  fi: "🇫🇮",
  pl: "🇵🇱",
  ru: "🇷🇺",
  cs: "🇨🇿",
  hu: "🇭🇺",
  ro: "🇷🇴",
  el: "🇬🇷",
  tr: "🇹🇷",
  ar: "🇸🇦",
  he: "🇮🇱",
  th: "🇹🇭",
  vi: "🇻🇳",
  id: "🇮🇩",
  ms: "🇲🇾",
  // Regions
  us: "🇺🇸",
  uk: "🇬🇧",
  jp: "🇯🇵",
  cn: "🇨🇳",
  kr: "🇰🇷",
  br: "🇧🇷",
  au: "🇦🇺",
  ca: "🇨🇦",
  mx: "🇲🇽",
  wor: "🌐",
  eu: "🇪🇺",
  ss: "🌐",
};

/**
 * Get the flag emoji for a language/region code.
 * Returns undefined if no match found.
 */
export function getFlag(langCode?: string): string | undefined {
  if (!langCode) return undefined;
  const normalized = langCode.toLowerCase().trim();
  return FLAG_MAP[normalized];
}
