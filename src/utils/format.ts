/** Parse EmulationStation date format (YYYYMMDDTHHMMSS) to readable string */
export function formatReleaseDate(date?: string): string {
  if (!date || date.length < 8) return "";
  const year = date.slice(0, 4);
  const month = date.slice(4, 6);
  const day = date.slice(6, 8);
  return `${year}-${month}-${day}`;
}

/** Format rating (0-1 float string) as percentage or stars */
export function formatRating(rating?: string): string {
  if (!rating) return "";
  const val = parseFloat(rating);
  if (isNaN(val)) return "";
  return `${Math.round(val * 100)}%`;
}

/** Convert a readable date (YYYY-MM-DD) back to ES format (YYYYMMDDTHHMMSS) */
export function toESDate(dateStr: string): string {
  const cleaned = dateStr.replace(/-/g, "");
  if (cleaned.length === 8) return `${cleaned}T000000`;
  return cleaned;
}

/** Bytes as KB / MB / GB (sizes of copies) */
export function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
  return `${Math.ceil(bytes / 1024)} KB`;
}

/** "2 days ago" / "hace 2 días" for a moment (ms since epoch), in the given language */
export function formatAgo(ms: number, locale: string, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const minutes = Math.round((ms - now) / 60000);
  if (Math.abs(minutes) < 60) return rtf.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return rtf.format(hours, "hour");
  const days = Math.round(hours / 24);
  if (Math.abs(days) < 30) return rtf.format(days, "day");
  const months = Math.round(days / 30);
  if (Math.abs(months) < 12) return rtf.format(months, "month");
  return rtf.format(Math.round(days / 365), "year");
}

/** EmulationStation date (YYYYMMDDTHHMMSS, local time) to ms since epoch; null if it isn't one */
export function esDateToMs(date?: string | null): number | null {
  const m = date?.match(/^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})/);
  if (!m) return null;
  return new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]).getTime();
}
