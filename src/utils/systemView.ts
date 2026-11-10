import type { AppSettings, SystemView, ViewMode } from "../types";
import { useAppStore } from "../store/useAppStore";
import { getActiveProfile } from "./profiles";

/*
 * Vista de cada sistema en el perfil activo (su ventanita de ajustes): medio de la cuadrícula,
 * forma de la tarjeta y medios de la portada y del fondo del detalle. Lo que no se ha elegido
 * es «automático». Se guarda en el perfil (`system_views`), así cada perfil tiene la suya.
 */

/** Clave de un sistema en `system_views` (como `system_key` en Rust) */
export const systemKey = (name: string) => name.toLowerCase().replace(/[^a-z0-9]/g, "");

/** La vista de un sistema en el perfil activo (vacía si no se ha tocado) */
export function getSystemView(settings: AppSettings, systemName: string | undefined): SystemView {
  if (!systemName) return {};
  return getActiveProfile(settings)?.system_views?.[systemKey(systemName)] ?? {};
}

/** Medio que enseña la cuadrícula de un sistema: el suyo o, si no tiene, el global */
export function effectiveGridField(settings: AppSettings, systemName: string | undefined): string {
  return getSystemView(settings, systemName).grid_media || settings.grid_media_field || "image";
}

/** Si la cuadrícula de un sistema enseña cajas 3D: lo elegido en el sistema o, si no, lo global */
export function boxes3dOn(settings: AppSettings, systemName: string | undefined): boolean {
  const own = getSystemView(settings, systemName).box3d;
  return own ? own === "on" : !!settings.grid_boxes_3d;
}

/**
 * Medio cuya forma se mide para las tarjetas: con cajas 3D (en la cuadrícula o en el carrusel)
 * el frente es siempre la carátula 2D, así que se mide esa; si no, el medio de la cuadrícula
 */
export function shapeField(settings: AppSettings, systemName: string | undefined, viewMode: ViewMode): string {
  if (viewMode === "carousel" || boxes3dOn(settings, systemName)) return "image";
  return effectiveGridField(settings, systemName);
}

/** Cambia la vista de un sistema en el perfil activo (se guarda sola, con una pequeña espera) */
export function setSystemView(systemName: string, changes: Partial<SystemView>) {
  const { settings, patchSettings } = useAppStore.getState();
  const active = getActiveProfile(settings);
  if (!active) return;
  const key = systemKey(systemName);
  const views = { ...(active.system_views ?? {}) };
  const view: SystemView = { ...(views[key] ?? {}), ...changes };
  // Lo que vuelve a «automático» no se guarda
  for (const k of Object.keys(view) as (keyof SystemView)[]) {
    if (!view[k]) delete view[k];
  }
  if (Object.keys(view).length === 0) delete views[key];
  else views[key] = view;
  patchSettings({
    profiles: (settings.profiles ?? []).map((p) =>
      p.id === active.id ? { ...p, system_views: Object.keys(views).length > 0 ? views : undefined } : p
    ),
  });
}
