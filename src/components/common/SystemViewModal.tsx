import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { GRID_MEDIA_OPTIONS, type SystemInfo, type SystemView } from "../../types";
import { getSystemView, setSystemView } from "../../utils/systemView";
import { getSystemAspectRatio } from "../../utils/cartridgeSizes";

interface SystemViewModalProps {
  open: boolean;
  system: SystemInfo | null;
  onClose: () => void;
}

/** Formas de tarjeta que se pueden elegir (además de «automático») */
const RATIOS = [
  { value: "3/4", label: "3:4" },
  { value: "7/12", label: "7:12 (UMD)" },
  { value: "27/38", label: "27:38 (DVD)" },
  { value: "27/33", label: "27:33 (Blu-ray)" },
  { value: "27/34", label: "27:34 (Switch)" },
  { value: "27/32", label: "27:32" },
  { value: "1/1", label: "1:1" },
  { value: "4/3", label: "4:3" },
  { value: "16/9", label: "16:9" },
  { value: "16/5", label: "16:5" },
];

/**
 * Ajustes de vista de un sistema en el perfil activo: medio de la cuadrícula, forma de la
 * tarjeta y medios de la portada y del fondo del detalle del juego. Cada cambio se aplica y se
 * guarda al momento; «Automático» es lo de siempre.
 */
export function SystemViewModal({ open, system, onClose }: SystemViewModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const mediaShape = useAppStore((s) => s.mediaShape);
  if (!open || !system) return null;

  const view = getSystemView(settings, system.name);
  const set = (changes: Partial<SystemView>) => setSystemView(system.name, changes);
  const mediaLabel = (id: string) => t((GRID_MEDIA_OPTIONS.find((o) => o.id === id)?.labelKey ?? id) as TranslationKeys);
  // Lo que usa «automático» en cada caso, para decirlo en la propia opción
  const globalGrid = mediaLabel(settings.grid_media_field || "image");
  const measured = mediaShape && mediaShape.system === system.path ? `${mediaShape.width}×${mediaShape.height}` : getSystemAspectRatio(system.name).replace("/", ":");

  const select = (value: string | undefined, onChange: (v: string) => void, auto: string, options: { value: string; label: string }[]) => (
    <select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value)}
      className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
    >
      <option value="">{t("systemView.auto", { what: auto })}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
  const mediaOptions = GRID_MEDIA_OPTIONS.map((o) => ({ value: o.id, label: t(o.labelKey as TranslationKeys) }));

  const row = (label: string, help: string, control: React.ReactNode) => (
    <label className="block">
      <span className="text-xs text-zinc-400">{label}</span>
      <div className="mt-1">{control}</div>
      <span className="block text-[11px] text-zinc-500 mt-1">{help}</span>
    </label>
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onMouseDown={onClose}>
      <div
        className="bg-zinc-800 border border-zinc-600 rounded-lg w-[480px] max-w-[calc(100vw-2rem)] flex flex-col mx-4"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("systemView.title", { system: system.full_name })}</h3>
          <p className="text-xs text-zinc-500 mt-1">{t("systemView.help")}</p>
        </div>
        <div className="px-5 py-4 space-y-4">
          {row(t("systemView.gridMedia"), t("systemView.gridMediaHelp"), select(view.grid_media, (v) => set({ grid_media: v }), globalGrid, mediaOptions))}
          {row(
            t("systemView.box3d"),
            t("systemView.box3dHelp"),
            select(view.box3d, (v) => set({ box3d: (v || undefined) as SystemView["box3d"] }), t(settings.grid_boxes_3d ? "systemView.box3dOn" : "systemView.box3dOff"), [
              { value: "on", label: t("systemView.box3dOn") },
              { value: "off", label: t("systemView.box3dOff") },
            ])
          )}
          {row(t("systemView.ratio"), t("systemView.ratioHelp"), select(view.aspect_ratio, (v) => set({ aspect_ratio: v }), measured, RATIOS))}
          {row(t("systemView.cover"), t("systemView.coverHelp"), select(view.cover, (v) => set({ cover: v }), t("systemView.coverAuto"), mediaOptions))}
          {row(t("systemView.hero"), t("systemView.heroHelp"), select(view.hero, (v) => set({ hero: v }), t("systemView.heroAuto"), mediaOptions))}
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={() => set({ grid_media: undefined, box3d: undefined, aspect_ratio: undefined, cover: undefined, hero: undefined })}
            disabled={Object.keys(view).length === 0}
            className="px-4 py-2 text-sm text-zinc-300 rounded border border-zinc-600 hover:bg-zinc-700 disabled:opacity-40"
          >
            {t("systemView.reset")}
          </button>
          <button onClick={onClose} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500">
            {t("dialog.ok")}
          </button>
        </div>
      </div>
    </div>
  );
}
