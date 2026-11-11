import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { HomeIcon, SearchIcon, SettingsIcon } from "../common/Icons";
import { SEARCH_SHORTCUT } from "../common/GlobalSearch";
import { Tooltip } from "../common/Tooltip";
import { TaskProgress } from "../common/TaskProgress";
import { sidebarSystems } from "../../utils/systems";

export function Sidebar() {
  const systems = useAppStore((s) => s.systems);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const selectSystem = useAppStore((s) => s.selectSystem);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const setSearchOpen = useAppStore((s) => s.setSearchOpen);
  const goHome = useAppStore((s) => s.goHome);
  const loading = useAppStore((s) => s.loading);
  const scanProgress = useAppStore((s) => s.scanProgress);
  const pendingStructure = useAppStore((s) => s.pendingStructure);
  const onlyWithGames = useAppStore((s) => s.onlySystemsWithGames);
  const toggleOnlyWithGames = useAppStore((s) => s.toggleOnlySystemsWithGames);
  const { t } = useTranslation();

  const visibleSystems = sidebarSystems(systems, onlyWithGames, selectedSystem?.path);

  const isScanning = loading && (scanProgress !== null || systems.length === 0);

  return (
    <aside className="w-28 shrink-0 overflow-hidden bg-zinc-950 border-r border-zinc-800 flex flex-col h-full">
      {/* Inicio (las estadísticas de la biblioteca) y buscador global: un juego entre todos los
          sistemas (también con Cmd+K / Ctrl+K) */}
      {systems.length > 0 && (
        <div className="border-b border-zinc-800 h-12 shrink-0 flex items-center justify-center gap-0.5">
          <Tooltip text={t("stats.home")}>
            <button
              onClick={goHome}
              aria-label={t("stats.home")}
              data-shot="home"
              className={`p-1.5 rounded-md transition-colors ${
                selectedSystem ? "text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800" : "text-indigo-300 bg-zinc-800"
              }`}
            >
              <HomeIcon size={16} />
            </button>
          </Tooltip>
          <Tooltip text={t("globalSearch.open", { shortcut: SEARCH_SHORTCUT })}>
            <button
              onClick={() => setSearchOpen(true)}
              aria-label={t("globalSearch.open", { shortcut: SEARCH_SHORTCUT })}
              className="flex items-center gap-1.5 px-2 py-1.5 text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800 rounded-md transition-colors"
            >
              <SearchIcon size={16} />
              <span className="text-[10px] font-mono text-zinc-500">{SEARCH_SHORTCUT}</span>
            </button>
          </Tooltip>
        </div>
      )}
      <nav className="flex-1 overflow-y-auto overflow-x-hidden py-3">
        {isScanning ? (
          // Escaneando: qué sistema está leyendo y cuántos van (la barra es estrecha: compacto)
          <div className="px-2 py-4">
            <TaskProgress progress={scanProgress} label={t("sidebar.scanning")} compact />
          </div>
        ) : visibleSystems.length === 0 ? (
          // Sin sistemas que enseñar la barra queda vacía: lo explica la pantalla del centro
          null
        ) : (
          visibleSystems.map((system) => {
            const selected = selectedSystem?.path === system.path;
            const count = system.has_gamelist ? system.game_count : system.rom_count;
            return (
              <button
                key={system.path}
                onClick={() => {
                  selectSystem(system);
                  setCurrentView("browser");
                }}
                title={system.full_name}
                className={`group relative w-full px-4 py-3 flex justify-center transition-colors ${
                  selected ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"
                }`}
              >
                {/* Franja violeta a la izquierda del sistema elegido (como GOG) */}
                {selected && <span className="absolute left-0 top-1 bottom-1 w-[3px] rounded-r bg-indigo-500" />}
                {/* Logo centrado (el nombre completo sale al pasar el ratón); si no hay logo,
                    el nombre corto del sistema (el de su carpeta). Las marcas van pegadas al logo. */}
                <span className="relative h-7 flex items-center">
                  {hasSystemLogo(system.name) ? (
                    <SystemLogo
                      name={system.name}
                      width={56}
                      maxHeight={22}
                      // Los no elegidos, algo apagados
                      className={selected ? "" : "opacity-60 group-hover:opacity-100 transition-opacity"}
                    />
                  ) : (
                    <span className="text-sm font-semibold uppercase tracking-wide">{system.name}</span>
                  )}
                  {/* Número de juegos: insignia pequeña en la esquina superior derecha */}
                  {count > 0 && (
                    <span
                      className={`absolute -top-2 -right-4 min-w-[18px] text-center text-[9px] leading-none font-semibold px-1 py-0.5 rounded-full ${
                        selected ? "bg-indigo-600 text-white" : "bg-zinc-700 text-zinc-300"
                      }`}
                    >
                      {count}
                    </span>
                  )}
                  {/* Marca: este sistema está en la otra estructura y se puede migrar */}
                  {pendingStructure?.systems.includes(system.name) && (
                    <span
                      className="absolute -top-1.5 -left-3 w-2 h-2 rounded-full bg-amber-400"
                      title={t("structure.mark", {
                        kind: pendingStructure.source.kind === "esde" ? "ES-DE" : t("profiles.kindClassic"),
                      })}
                    />
                  )}
                </span>
              </button>
            );
          })
        )}
      </nav>

      <div className="border-t border-zinc-800 h-12 shrink-0 flex items-center justify-center gap-2">
        {/* Solo el icono; el texto sale al pasar el ratón */}
        <Tooltip text={t("sidebar.settings")}>
        <button
          onClick={() => setCurrentView("settings")}
          aria-label={t("sidebar.settings")}
          className="p-2 text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800 rounded-md transition-colors"
        >
          <SettingsIcon size={18} />
        </button>
        </Tooltip>
        {/* Interruptor: solo sistemas con juegos (por defecto) / todos */}
        <Tooltip text={t(onlyWithGames ? "sidebar.showAllSystems" : "sidebar.showOnlyWithGames")}>
        <button
          role="switch"
          aria-checked={onlyWithGames}
          onClick={toggleOnlyWithGames}
          aria-label={t("sidebar.onlyWithGames")}
          className={`relative w-8 h-[18px] rounded-full transition-colors ${onlyWithGames ? "bg-indigo-600" : "bg-zinc-700"}`}
        >
          <span
            className={`absolute top-[3px] w-3 h-3 rounded-full bg-white transition-all ${onlyWithGames ? "left-[17px]" : "left-[3px]"}`}
          />
        </button>
        </Tooltip>
      </div>
    </aside>
  );
}
