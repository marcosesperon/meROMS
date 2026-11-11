import { openUrl } from "@tauri-apps/plugin-opener";

// Direcciones del proyecto que se usan en varios sitios (Acerca de, aviso de donativo).
// Si cambia alguna, se toca solo aquí. Son las mismas que en los demás proyectos del autor.
export const REPOSITORY_URL = "https://github.com/marcosesperon/meROMS";
export const WEBSITE_URL = "https://marcosesperon.github.io/meROMS/";
export const DONATION_URL = "https://buymeacoffee.com/marcosesperon";
/** Where the RetroAchievements web API key is (account settings) */
export const RA_SETTINGS_URL = "https://retroachievements.org/settings";
/** El nombre del autor va tal cual: no se traduce */
export const AUTHOR = "Marcos Esperón";

/** Abre una dirección en el navegador del sistema (no dentro de la ventana de la app) */
export function openExternal(url: string): void {
  openUrl(url).catch((e) => console.error("Failed to open link:", e));
}

// Aviso de donativo: como mucho uno cada 7 días, y el primero a los 7 días del primer arranque
const DONATION_INTERVAL_SECONDS = 7 * 24 * 3600;

/**
 * Decide si toca enseñar el aviso de donativo y qué fecha hay que guardar.
 * `lastAt` es lo guardado en los ajustes: vacío la primera vez (entonces solo se empieza a
 * contar), y luego la última vez que se enseñó.
 */
export function donationPromptDue(lastAt: number | undefined, now: number): { show: boolean; saveAt?: number } {
  if (!lastAt) return { show: false, saveAt: now };
  if (now - lastAt >= DONATION_INTERVAL_SECONDS) return { show: true, saveAt: now };
  return { show: false };
}

/** Página de la última versión publicada (instaladores de todas las plataformas) */
export const RELEASES_URL = `${REPOSITORY_URL}/releases/latest`;
