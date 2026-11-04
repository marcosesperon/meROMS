import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { SettingsIcon } from "../common/Icons";

export function Sidebar() {
  const systems = useAppStore((s) => s.systems);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const selectSystem = useAppStore((s) => s.selectSystem);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const loading = useAppStore((s) => s.loading);
  const scanProgress = useAppStore((s) => s.scanProgress);
  const pendingStructure = useAppStore((s) => s.pendingStructure);
  const { t } = useTranslation();

  const isScanning = loading && scanProgress !== null;

  return (
    <aside className="w-48 bg-zinc-950 border-r border-zinc-800 flex flex-col h-full">
      <nav className="flex-1 overflow-y-auto py-3">
        {isScanning ? (
          <div className="px-4 py-6 flex flex-col items-center gap-3">
            {/* Spinner */}
            <svg className="animate-spin h-8 w-8 text-indigo-400" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>

            {/* Progress text */}
            <p className="text-xs text-zinc-400 text-center">
              {t("sidebar.scanning")}
            </p>
            <p className="text-sm text-zinc-200 text-center truncate w-full">
              {scanProgress.currentName}
            </p>

            {/* Progress bar */}
            <div className="w-full bg-zinc-800 rounded-full h-1.5 overflow-hidden">
              <div
                className="bg-indigo-500 h-full rounded-full transition-all duration-200"
                style={{ width: `${(scanProgress.current / scanProgress.total) * 100}%` }}
              />
            </div>
            <p className="text-[10px] text-zinc-500">
              {scanProgress.current} / {scanProgress.total}
            </p>
          </div>
        ) : systems.length === 0 ? (
          <p className="text-zinc-500 text-sm px-4 py-2">{t("sidebar.noSystems")}</p>
        ) : (
          systems.map((system) => {
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

      <div className="border-t border-zinc-800 h-12 shrink-0 flex items-center px-3">
        {/* Solo el icono; el texto sale al pasar el ratón */}
        <button
          onClick={() => setCurrentView("settings")}
          title={t("sidebar.settings")}
          aria-label={t("sidebar.settings")}
          className="p-2 text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800 rounded-md transition-colors"
        >
          <SettingsIcon size={18} />
        </button>
      </div>
    </aside>
  );
}
