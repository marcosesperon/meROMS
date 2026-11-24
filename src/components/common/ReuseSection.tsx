import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import type { ReusableMedia } from "../../types";
import { resolveImageSrc } from "../../utils/images";
import { REUSE_REASON_KEYS } from "../../utils/mediaReuse";

interface ReuseSectionProps {
  systemPath: string;
  items: ReusableMedia[];
  /** Nombre del tipo de medio de un campo (el de la ventana de sobrantes) */
  kindLabel: (field: string) => string;
  /** Incorporar lo marcado, con lo que se queda de cada uno */
  onApply: (items: ReusableMedia[], keepNew: (item: ReusableMedia) => boolean) => void;
  busy: boolean;
}

const fileKey = (i: ReusableMedia) => `${i.path}|${i.game_path}|${i.field}`;

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Miniatura de un medio: la imagen, o un icono para vídeos y manuales */
function Thumb({ path, systemPath, dimmed }: { path: string; systemPath: string; dimmed?: boolean }) {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  const icon = ["mp4", "mkv", "avi", "mov", "webm", "wmv"].includes(ext) ? "▶" : ext === "pdf" ? "PDF" : null;
  const cls = `w-12 h-12 shrink-0 rounded bg-zinc-900 ${dimmed ? "opacity-40" : ""}`;
  if (icon) return <span className={`${cls} flex items-center justify-center text-xs text-zinc-500`}>{icon}</span>;
  return <img src={resolveImageSrc(path, systemPath)} alt="" className={`${cls} object-contain`} />;
}

/** «300×420 · 75 KB», lo que se sepa */
function sizeText(width: number | null, height: number | null, bytes: number): string {
  return [width && height ? `${width}×${height}` : null, formatSize(bytes)].filter(Boolean).join(" · ");
}

/**
 * «Se pueden reaprovechar»: sobrantes que son de un juego (con otro nombre) y ficheros de
 * referencias rotas encontrados en otro sitio. Cada fila dice de qué juego y tipo es y por qué se
 * sabe; si el juego ya tiene uno, se enseñan los dos y se elige con cuál quedarse (la propuesta:
 * el de más resolución). «Incorporar» quita lo que no se queda y pone el otro con el nombre bueno.
 */
export function ReuseSection({ systemPath, items, kindLabel, onApply, busy }: ReuseSectionProps) {
  const { t } = useTranslation();
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [keep, setKeep] = useState<Map<string, boolean>>(new Map());
  // Lo nuevo (otra búsqueda, uno asignado a mano) empieza marcado y con la propuesta
  const seen = useRef<Set<string>>(new Set());
  useEffect(() => {
    const fresh = items.map(fileKey).filter((k) => !seen.current.has(k));
    fresh.forEach((k) => seen.current.add(k));
    const live = new Set(items.map(fileKey));
    setChecked((prev) => new Set([...[...prev].filter((k) => live.has(k)), ...fresh]));
    setKeep((prev) => new Map(items.map((i) => [fileKey(i), prev.get(fileKey(i)) ?? i.keep_new])));
  }, [items]);

  if (items.length === 0) return null;
  const keepNew = (i: ReusableMedia) => keep.get(fileKey(i)) ?? i.keep_new;
  const marked = items.filter((i) => checked.has(fileKey(i)));
  const toggle = (k: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });
  const choose = (k: string, value: boolean) => setKeep((prev) => new Map(prev).set(k, value));

  return (
    <div className="border-b border-zinc-700/50" data-reuse-section>
      <div className="px-5 pt-3 pb-1 flex items-start gap-3">
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-emerald-400">{t("reuse.section", { count: items.length })}</p>
          <p className="text-xs text-zinc-500">{t("reuse.help")}</p>
        </div>
        <button
          onClick={() => onApply(marked, keepNew)}
          disabled={marked.length === 0 || busy}
          className="px-3 py-1.5 text-xs bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
        >
          {busy ? "..." : t("reuse.apply", { count: marked.length })}
        </button>
      </div>
      <div className="divide-y divide-zinc-700/30">
        {items.map((item) => {
          const k = fileKey(item);
          const mine = keepNew(item);
          return (
            <div key={k} className="flex items-center gap-3 px-5 py-2">
              <input
                type="checkbox"
                checked={checked.has(k)}
                onChange={() => toggle(k)}
                className="rounded border-zinc-600 bg-zinc-700 text-emerald-600 shrink-0"
              />
              <Thumb path={item.path} systemPath={systemPath} dimmed={!mine} />
              <div className="min-w-0 flex-1">
                <p className="text-sm text-zinc-200 truncate">
                  {item.game_name} <span className="text-zinc-500">· {kindLabel(item.field)}</span>
                </p>
                <p className="text-xs text-zinc-500 truncate" title={item.relative}>
                  {item.relative} · {sizeText(item.width, item.height, item.size)}
                </p>
                <p className="text-[11px] text-zinc-500">
                  {t(REUSE_REASON_KEYS[item.reason])}
                  {item.fixes_broken ? ` · ${t("reuse.fixesBroken")}` : ""}
                  {!item.current ? ` · ${t("reuse.fillsGap")}` : ""}
                </p>
              </div>
              {/* El juego ya tiene uno: los dos, y con cuál se queda */}
              {item.current && (
                <div className="flex items-center gap-2 shrink-0">
                  <div className="flex flex-col gap-1 text-[11px]">
                    <button
                      onClick={() => choose(k, true)}
                      className={`px-2 py-0.5 rounded ${mine ? "bg-emerald-600/80 text-white" : "bg-zinc-800 text-zinc-400 hover:text-zinc-200"}`}
                    >
                      {t("reuse.keepThis")}
                    </button>
                    <button
                      onClick={() => choose(k, false)}
                      className={`px-2 py-0.5 rounded ${!mine ? "bg-zinc-600 text-white" : "bg-zinc-800 text-zinc-400 hover:text-zinc-200"}`}
                    >
                      {t("reuse.keepCurrent")}
                    </button>
                  </div>
                  <div className="flex flex-col items-center" title={item.current.relative}>
                    <Thumb path={item.current.path} systemPath={systemPath} dimmed={mine} />
                    <span className="text-[10px] text-zinc-500 mt-0.5">{sizeText(item.current.width, item.current.height, item.current.size)}</span>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
