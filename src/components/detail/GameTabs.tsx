import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { Game } from "../../types";
import type { GalleryItem } from "../../hooks/useGameDetails";
import { openManual } from "../../commands";
import { resolveImageSrc, absoluteMediaPath } from "../../utils/images";
import { showMessage } from "../../utils/dialogs";
import { PdfViewer } from "../common/PdfViewer";
import { GameMediaViewer } from "./GameMediaViewer";
import { SavesTab } from "./SavesTab";
import { MetadataTab } from "./MetadataTab";

type Tab = "media" | "manual" | "saves" | "meta";
const TABS: { id: Tab; label: TranslationKeys }[] = [
  { id: "media", label: "tabs.media" },
  { id: "manual", label: "tabs.manual" },
  { id: "saves", label: "tabs.saves" },
  { id: "meta", label: "tabs.metadata" },
];

interface GameTabsProps {
  game: Game;
  systemPath: string;
  media: GalleryItem[];
  shown?: GalleryItem;
  onPick: (index: number) => void;
  onFailed: (url: string) => void;
  /** Ocupar el alto del hueco (ficha en dos columnas) en vez de crecer con el contenido */
  fill?: boolean;
}

/**
 * Pestañas de la ficha a pantalla completa: medios, manual (visor de PDF), datos guardados y
 * metadatos. La elegida se mantiene al pasar de un juego a otro.
 */
export function GameTabs({ game, systemPath, media, shown, onPick, onFailed, fill = false }: GameTabsProps) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("media");
  const manualUrl = resolveImageSrc(game.manual, systemPath);
  // Sin medios que enseñar, mejor empezar por los metadatos que por una pestaña vacía
  useEffect(() => {
    if (tab === "media" && media.length === 0) setTab("meta");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.path]);

  async function openExternal() {
    try {
      await openManual(absoluteMediaPath(game.manual!, systemPath));
    } catch (e) {
      await showMessage(String(e), "error");
    }
  }

  const empty = (text: string) => <p className="py-12 text-center text-sm text-zinc-500">{text}</p>;

  return (
    <div className={`flex flex-col ${fill ? "h-full min-h-0" : ""}`}>
      <div className="flex gap-1 border-b border-zinc-700/70 shrink-0">
        {TABS.map((tb) => (
          <button
            key={tb.id}
            onClick={() => setTab(tb.id)}
            data-shot={`tab-${tb.id}`}
            className={`px-4 py-2 text-sm -mb-px border-b-2 transition-colors ${
              tab === tb.id ? "border-indigo-400 text-zinc-100" : "border-transparent text-zinc-400 hover:text-zinc-200"
            }`}
          >
            {t(tb.label)}
          </button>
        ))}
      </div>
      <div className={`pt-4 ${fill ? "flex-1 min-h-0" : "min-h-[320px]"}`}>
        {tab === "media" &&
          (shown ? <GameMediaViewer media={media} shown={shown} onPick={onPick} onFailed={onFailed} large fit={fill} /> : empty(t("tabs.noMedia")))}
        {tab === "manual" &&
          (manualUrl ? (
            <div className={fill ? "h-full" : "h-[70vh]"}>
              <PdfViewer url={manualUrl} onOpenExternal={openExternal} />
            </div>
          ) : (
            empty(t("tabs.noManual"))
          ))}
        {tab === "saves" && <SavesTab game={game} systemPath={systemPath} />}
        {tab === "meta" && <MetadataTab game={game} systemPath={systemPath} />}
      </div>
    </div>
  );
}
