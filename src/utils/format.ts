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
