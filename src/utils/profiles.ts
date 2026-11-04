import type { AppSettings, Profile, StructureKind } from "../types";

// Ayudas pequeñas para trabajar con los perfiles guardados en los ajustes.

/** Perfil en uso (o undefined si todavía no hay ninguno). */
export function getActiveProfile(settings: AppSettings): Profile | undefined {
  const profiles = settings.profiles ?? [];
  return profiles.find((p) => p.id === settings.active_profile_id) ?? profiles[0];
}

/** Id único para un perfil nuevo; no hace falta nada más sofisticado. */
export function newProfileId(): string {
  return `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function createProfile(name: string, kind: StructureKind): Profile {
  return { id: newProfileId(), name, kind, rom_root_path: "" };
}

/**
 * ¿Tiene el perfil lo mínimo para funcionar?
 * Clásica: carpeta de ROMs. ES-DE: además, la carpeta de ES-DE.
 */
export function isProfileComplete(profile: Profile | undefined): boolean {
  if (!profile || !profile.rom_root_path) return false;
  return profile.kind !== "esde" || !!profile.esde_home_path;
}

/**
 * Clave que cambia cuando cambia algo que obliga a recargar los sistemas
 * (otro perfil, otra carpeta, otra estructura…).
 */
export function profileLocationKey(profile: Profile | undefined): string {
  if (!profile) return "";
  return [profile.id, profile.kind, profile.rom_root_path, profile.esde_home_path ?? "", profile.media_dir ?? ""].join("|");
}

/**
 * Si la carpeta de medios detectada es la de por defecto de ES-DE (<ES-DE>/downloaded_media),
 * el perfil no necesita guardarla. Se compara con «/» para que las rutas de Windows («\») también.
 */
export function isDefaultEsdeMedia(paths: { esde_home: string; media_dir: string }): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  return norm(paths.media_dir) === `${norm(paths.esde_home)}/downloaded_media`;
}
