import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { searchGames, type SearchHit } from "../../commands";
import { resolveImageSrc } from "../../utils/images";
import { SearchIcon } from "./Icons";
import { SystemLogo } from "./SystemLogo";

/** El atajo tal como se escribe en cada sistema */
export const SEARCH_SHORTCUT = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘K" : "Ctrl+K";

// Espera tras la última tecla antes de buscar
const DEBOUNCE_MS = 150;

/**
 * Buscador global (⌘K / Ctrl+K o la lupa de la barra lateral): un juego entre todos los
 * sistemas del perfil, por su nombre o el de su ROM. Al elegirlo se va a su sistema, queda
 * seleccionado y, con «Al pulsar un juego» = ficha a pantalla completa, se abre la ficha.
 */
export function GlobalSearch() {
  const open = useAppStore((s) => s.searchOpen);
  if (!open) return null;
  return <SearchPalette />;
}

function SearchPalette() {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const setSearchOpen = useAppStore((s) => s.setSearchOpen);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

  // Buscar al dejar de escribir; una respuesta que llega tarde (texto ya cambiado) no cuenta
  useEffect(() => {
    const text = query.trim();
    if (!text) {
      setHits([]);
      setSearching(false);
      return;
    }
    let cancelled = false;
    setSearching(true);
    const timer = setTimeout(() => {
      searchGames(
        systems.filter((s) => s.has_gamelist).map((s) => s.path),
        text
      )
        .then((r) => {
          if (cancelled) return;
          setHits(r);
          setActive(0);
        })
        .catch(() => !cancelled && setHits([]))
        .finally(() => !cancelled && setSearching(false));
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query, systems]);

  // Grupos por sistema (los resultados ya vienen agrupados), con su posición en la lista plana
  const groups = useMemo(() => {
    const out: { system: SearchHit; items: { hit: SearchHit; index: number }[] }[] = [];
    hits.forEach((hit, index) => {
      const last = out[out.length - 1];
      if (last && last.system.system_path === hit.system_path) last.items.push({ hit, index });
      else out.push({ system: hit, items: [{ hit, index }] });
    });
    return out;
  }, [hits]);

  // Que el elegido con las flechas se vea
  useEffect(() => {
    listRef.current?.querySelector(`[data-hit="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  async function choose(hit: SearchHit) {
    setSearchOpen(false);
    const store = useAppStore.getState();
    const system = store.systems.find((s) => s.path === hit.system_path);
    if (!system) return;
    store.closeFullView();
    store.closeEditor();
    if (store.multiSelect) store.setMultiSelect(false);
    if (store.selectedSystem?.path !== system.path) {
      store.selectSystem(system);
      // Esperar a que estén sus juegos (como mucho unos segundos)
      const end = Date.now() + 8000;
      while (Date.now() < end) {
        const s = useAppStore.getState();
        if (s.games.length > 0 && !s.loading && s.selectedSystem?.path === system.path) break;
        await new Promise((r) => setTimeout(r, 80));
      }
    }
    // El juego puede estar oculto por la búsqueda o los filtros de ese sistema: se quitan
    const s = useAppStore.getState();
    s.setSearchQuery("");
    s.clearFilters();
    const game = s.games.find((g) => g.path === hit.game_path);
    if (!game) return;
    s.selectGame(game);
    setTimeout(() => {
      document.querySelector(`[data-game-path="${CSS.escape(game.path)}"]`)?.scrollIntoView({ block: "center" });
    }, 150);
    if (s.settings.click_action === "full") s.openFullView();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      setSearchOpen(false);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(hits.length - 1, a + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === "Enter" && hits[active]) {
      e.preventDefault();
      choose(hits[active]);
    }
  }

  return (
    <div className="fixed inset-0 z-[80] bg-black/60 flex items-start justify-center pt-[12vh]" onMouseDown={() => setSearchOpen(false)}>
      <div
        className="w-[620px] max-w-[calc(100vw-2rem)] bg-zinc-900 border border-zinc-700 rounded-xl shadow-2xl shadow-black/60 overflow-hidden"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <div className="flex items-center gap-3 px-4 border-b border-zinc-800">
          <SearchIcon size={18} className="text-zinc-500 shrink-0" />
          <input
            ref={inputRef}
            data-shot="global-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("globalSearch.placeholder")}
            className="flex-1 bg-transparent py-4 text-base text-zinc-100 placeholder-zinc-500 focus:outline-none"
          />
          {searching && <span className="text-xs text-zinc-500">…</span>}
        </div>

        <div ref={listRef} className="max-h-[56vh] overflow-y-auto">
          {query.trim() && !searching && hits.length === 0 && (
            <p className="px-4 py-8 text-center text-sm text-zinc-500">{t("globalSearch.none")}</p>
          )}
          {groups.map((group) => (
            <div key={group.system.system_path} className="pb-1">
              {/* Cabecera del sistema: fija arriba mientras se recorren sus resultados */}
              <div className="sticky top-0 z-10 flex items-center gap-3 px-4 py-2 bg-zinc-800/95 border-y border-zinc-700/60 backdrop-blur">
                <SystemLogo name={group.system.system_name} width={72} maxHeight={22} />
                <span className="flex-1 text-sm font-semibold text-zinc-100 truncate">{group.system.system_full_name}</span>
                <span className="text-[11px] text-zinc-400 tabular-nums">{group.items.length}</span>
              </div>
              {group.items.map(({ hit, index }) => {
                const cover = hit.cover ? resolveImageSrc(hit.cover, hit.system_path) : undefined;
                return (
                  <button
                    key={`${hit.system_path}|${hit.game_path}`}
                    data-hit={index}
                    onMouseMove={() => setActive(index)}
                    onClick={() => choose(hit)}
                    // Con sangría: cuelgan de la cabecera de su sistema
                    className={`w-full flex items-center gap-3 pl-10 pr-4 py-1.5 text-left ${index === active ? "bg-indigo-600/25" : ""}`}
                  >
                    <span className="w-8 h-10 shrink-0 rounded-sm bg-zinc-800 overflow-hidden flex items-center justify-center">
                      {cover && <img src={cover} alt="" loading="lazy" className="w-full h-full object-cover" />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block text-sm text-zinc-100 truncate">{hit.name}</span>
                      <span className="block text-[11px] text-zinc-500 font-mono truncate">{hit.game_path}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          ))}
        </div>

        <div className="flex items-center gap-4 px-4 py-2 border-t border-zinc-800 text-[11px] text-zinc-500">
          <span>{t("globalSearch.hintMove")}</span>
          <span>{t("globalSearch.hintOpen")}</span>
          <span>{t("globalSearch.hintClose")}</span>
          {hits.length > 0 && <span className="ml-auto">{t("globalSearch.count", { count: hits.length })}</span>}
        </div>
      </div>
    </div>
  );
}
