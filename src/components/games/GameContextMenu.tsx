import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useAppStore, type GameToolId } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { PLAY_STATUSES, type StatusChange } from "../../commands";
import type { Game } from "../../types";
import { STATUS_FLAGS, STATUS_META } from "../../utils/gameStatus";
import { absoluteMediaPath } from "../../utils/images";
import { removeGames } from "../../utils/collectionActions";
import { isArcadeSystem } from "../../utils/arcade";
import { showMessage } from "../../utils/dialogs";
import { CheckIcon, EyeIcon, EyeSlashIcon, FolderIcon, PencilIcon, PlayIcon, SearchIcon, StarIcon, TrashIcon } from "../common/Icons";

/** «Mostrar en Finder» / «en el Explorador» / «en la carpeta», según el sistema operativo */
function revealLabel(): TranslationKeys {
  const ua = navigator.userAgent;
  if (/Mac/i.test(ua)) return "menu.revealMac";
  if (/Win/i.test(ua)) return "menu.revealWin";
  return "menu.revealOther";
}

/**
 * Menú del botón derecho sobre un juego de la cuadrícula o de la tabla (`gameMenu` del store).
 * Sobre un juego: ver y jugar, marcas, sus herramientas y lo del fichero. Con selección múltiple
 * y el juego marcado: lo mismo que se puede hacer con los marcados. Sale donde está el ratón sin
 * salirse de la ventana; se cierra con un clic fuera, Esc, el scroll o al elegir algo.
 * Lleva `data-popup-open`: la ficha y el teclado de la lista no hacen caso mientras está abierto.
 */
export function GameContextMenu() {
  const menu = useAppStore((s) => s.gameMenu);
  const close = useAppStore((s) => s.closeGameMenu);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });

  // Que quepa: si se saldría por la derecha o por abajo, se abre hacia el otro lado
  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const { width, height } = ref.current.getBoundingClientRect();
    const x = menu.x + width > window.innerWidth - 8 ? Math.max(8, menu.x - width) : menu.x;
    const y = menu.y + height > window.innerHeight - 8 ? Math.max(8, window.innerHeight - 8 - height) : menu.y;
    setPos({ x, y });
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close();
    };
    // Al desplazar la lista el juego ya no está bajo el menú (el scroll del propio menú no cuenta)
    const onScroll = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    document.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
    };
  }, [menu, close]);

  if (!menu) return null;
  return createPortal(
    <div
      ref={ref}
      data-popup-open
      onContextMenu={(e) => e.preventDefault()}
      className="fixed z-[80] min-w-[230px] py-1 bg-zinc-800 border border-zinc-600 rounded-lg shadow-2xl shadow-black/60 text-sm select-none"
      style={{ left: pos.x || menu.x, top: pos.y || menu.y }}
    >
      <MenuBody path={menu.path} />
    </div>,
    document.body
  );
}

/** Lo que lleva el menú: el de los marcados o el de un juego */
function MenuBody({ path }: { path: string }) {
  const multiSelect = useAppStore((s) => s.multiSelect);
  const selectedPaths = useAppStore((s) => s.selectedPaths);
  const game = useAppStore((s) => s.games.find((g) => g.path === path));
  if (multiSelect && selectedPaths.has(path)) return <MarkedMenu paths={[...selectedPaths]} />;
  return game ? <SingleMenu game={game} /> : null;
}

/** Hay alguna fuente que pueda scrapear este sistema (como el botón Scrapear todo) */
function useHasScraper() {
  return useAppStore((s) => {
    const st = s.scraperStatus;
    return st.screenscraper || st.tgdb || st.launchbox || (!!s.selectedSystem && isArcadeSystem(s.selectedSystem.name));
  });
}

function SingleMenu({ game }: { game: Game }) {
  const { t } = useTranslation();
  const s = useAppStore.getState();
  const close = useAppStore((st) => st.closeGameMenu);
  const status = useAppStore((st) => st.statuses.get(game.path));
  const missed = useAppStore((st) => st.scrapeMisses.has(game.path));
  const systemPath = useAppStore((st) => st.selectedSystem?.path ?? "");
  const hasScraper = useHasScraper();
  const paths = [game.path];
  // Hacer algo y cerrar el menú
  const run = (action: () => unknown) => () => {
    close();
    void action();
  };
  const tool = (id: GameToolId) => run(() => s.openGameTool(id, paths));

  return (
    <>
      <MenuItem icon={<PlayIcon size={14} />} label={t("menu.play")} onClick={run(() => s.playGame(game))} />
      <MenuItem icon={<EyeIcon size={14} />} label={t("menu.fullView")} onClick={run(() => s.openFullView())} />
      <MenuItem icon={<PencilIcon size={14} />} label={t("menu.edit")} onClick={run(() => s.openEditor())} />
      <Separator />
      <MenuItem
        icon={<StarIcon size={14} className="text-yellow-400" />}
        label={t("menu.favorite")}
        checked={game.favorite === "true"}
        onClick={run(() => s.toggleGameFlag(game.path, "favorite"))}
      />
      <MenuItem icon={<EyeSlashIcon size={14} />} label={t("menu.hidden")} checked={game.hidden === "true"} onClick={run(() => s.toggleGameFlag(game.path, "hidden"))} />
      <StatusSubmenu paths={paths} current={(k) => (k === "status" ? status?.status ?? null : !!status?.[k])} />
      <MenuItem label={t(missed ? "menu.unmarkNotFound" : "menu.markNotFound")} onClick={run(() => s.setScrapeMisses(paths, !missed))} />
      <Separator />
      <MenuItem icon={<SearchIcon size={14} />} label={t("menu.scrape")} onClick={tool("scrape")} disabled={!hasScraper} />
      <MenuItem label={t("menu.rename")} onClick={tool("rename")} />
      <MenuItem label={t("menu.review")} onClick={tool("review")} />
      <MenuItem label={t("menu.normalize")} onClick={tool("normalize")} />
      <MenuItem label={t("menu.clean")} onClick={tool("clean")} />
      <MenuItem label={t("menu.versions")} onClick={tool("versions")} />
      <Separator />
      <MenuItem label={t("menu.move")} onClick={tool("move")} />
      <MenuItem
        icon={<FolderIcon size={14} />}
        label={t(revealLabel())}
        onClick={run(() => revealItemInDir(absoluteMediaPath(game.path, systemPath)).catch((e) => showMessage(String(e), "error")))}
      />
      <MenuItem icon={<TrashIcon size={14} />} label={t("menu.remove")} danger onClick={run(() => removeGames(paths))} />
    </>
  );
}

function MarkedMenu({ paths }: { paths: string[] }) {
  const { t } = useTranslation();
  const s = useAppStore.getState();
  const close = useAppStore((st) => st.closeGameMenu);
  const games = useAppStore((st) => st.games);
  const statuses = useAppStore((st) => st.statuses);
  const scrapeMisses = useAppStore((st) => st.scrapeMisses);
  const hasScraper = useHasScraper();
  const count = paths.length;
  const marked = games.filter((g) => paths.includes(g.path));
  // Si ya lo tienen todos, se ofrece quitarlo
  const allFavorite = marked.length > 0 && marked.every((g) => g.favorite === "true");
  const allHidden = marked.length > 0 && marked.every((g) => g.hidden === "true");
  const allMissed = paths.every((p) => scrapeMisses.has(p));
  const run = (action: () => unknown) => () => {
    close();
    void action();
  };

  return (
    <>
      <p className="px-3 pt-1 pb-1.5 text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{t("menu.markedTitle", { count })}</p>
      <MenuItem icon={<SearchIcon size={14} />} label={t("bulk.scrape", { count })} onClick={run(() => s.openTool({ id: "scrapeAll", gamePaths: paths }))} disabled={!hasScraper} />
      <MenuItem label={t("bulk.order", { count })} onClick={run(() => s.openTool({ id: "order", tab: "marked" }))} />
      <MenuItem label={t("bulk.versions", { count })} onClick={run(() => s.openGameTool("versions", paths))} />
      <MenuItem label={t("rename.marked", { count })} onClick={run(() => s.openGameTool("rename", paths))} />
      <Separator />
      <MenuItem
        icon={<StarIcon size={14} className="text-yellow-400" />}
        label={t(allFavorite ? "menu.favoriteOff" : "menu.favoriteOn")}
        onClick={run(() => s.setGameFlag(paths, "favorite", !allFavorite))}
      />
      <MenuItem icon={<EyeSlashIcon size={14} />} label={t(allHidden ? "menu.hiddenOff" : "menu.hiddenOn")} onClick={run(() => s.setGameFlag(paths, "hidden", !allHidden))} />
      <StatusSubmenu
        paths={paths}
        current={(k) =>
          k === "status"
            ? (() => {
                // El estado que tienen todos (si no es el mismo, ninguno sale marcado)
                const first = statuses.get(paths[0])?.status ?? null;
                return paths.every((p) => (statuses.get(p)?.status ?? null) === first) ? first : null;
              })()
            : paths.every((p) => !!statuses.get(p)?.[k])
        }
      />
      <MenuItem label={t(allMissed ? "scrapeMiss.bulkUnmark" : "scrapeMiss.bulkMark", { count })} onClick={run(() => s.setScrapeMisses(paths, !allMissed))} />
      <Separator />
      <MenuItem label={t("menu.moveMarked", { count })} onClick={run(() => s.openGameTool("move", paths))} />
      <MenuItem icon={<TrashIcon size={14} />} label={t("bulk.remove", { count })} danger onClick={run(() => removeGames(paths))} />
    </>
  );
}

/**
 * «Estado del juego ▸»: el estado de partida (uno; pulsar el puesto lo quita) y las marcas sueltas
 * (jugando, pendiente), como el menú de estado del panel. Se abre al pasar el ratón, hacia la
 * derecha o, si no cabe, hacia la izquierda.
 */
function StatusSubmenu({ paths, current }: { paths: string[]; current: (key: "status" | "playing" | "backlog") => string | boolean | null }) {
  const { t } = useTranslation();
  const setStatuses = useAppStore((s) => s.setStatuses);
  const close = useAppStore((s) => s.closeGameMenu);
  const [open, setOpen] = useState(false);
  const [left, setLeft] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);
  const status = current("status");

  const change = (c: StatusChange) => {
    close();
    setStatuses(paths, c).catch((e) => showMessage(String(e), "error"));
  };
  const show = () => {
    const rect = rowRef.current?.getBoundingClientRect();
    setLeft(!!rect && rect.right + 220 > window.innerWidth);
    setOpen(true);
  };

  return (
    <div ref={rowRef} className="relative" onMouseEnter={show} onMouseLeave={() => setOpen(false)}>
      <div className={`${ITEM} text-zinc-200 ${open ? "bg-zinc-700/70" : ""}`}>
        <span className="w-4" />
        <span className="flex-1">{t("menu.status")}</span>
        <span className="text-zinc-500">▸</span>
      </div>
      {open && (
        <div className={`absolute top-[-5px] ${left ? "right-full mr-0.5" : "left-full ml-0.5"} min-w-[210px] py-1 bg-zinc-800 border border-zinc-600 rounded-lg shadow-2xl shadow-black/60`}>
          {PLAY_STATUSES.map((st) => {
            const meta = STATUS_META[st];
            const active = status === st;
            return (
              <MenuItem key={st} icon={<meta.icon size={14} className={meta.color} />} label={t(meta.label)} checked={active} onClick={() => change({ status: active ? null : st })} />
            );
          })}
          <Separator />
          {STATUS_FLAGS.map((f) => {
            const active = !!current(f.key);
            return <MenuItem key={f.key} icon={<f.icon size={14} className={f.color} />} label={t(f.label)} checked={active} onClick={() => change({ [f.key]: !active })} />;
          })}
        </div>
      )}
    </div>
  );
}

const ITEM = "w-full flex items-center gap-2.5 px-3 py-1.5 text-left rounded-none";

function MenuItem({
  icon,
  label,
  onClick,
  checked,
  disabled,
  danger,
}: {
  icon?: ReactNode;
  label: string;
  onClick: () => void;
  checked?: boolean;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`${ITEM} disabled:opacity-40 disabled:cursor-default ${
        danger ? "text-red-300 hover:bg-red-600/20" : "text-zinc-200 hover:bg-zinc-700/70 disabled:hover:bg-transparent"
      }`}
    >
      {/* Hueco del icono siempre, para que los textos queden en columna */}
      <span className="w-4 flex items-center justify-center text-zinc-400 shrink-0">{icon}</span>
      <span className="flex-1 truncate">{label}</span>
      {checked && <CheckIcon size={13} className="text-indigo-300 shrink-0" />}
    </button>
  );
}

function Separator() {
  return <div className="my-1 border-t border-zinc-700" />;
}
