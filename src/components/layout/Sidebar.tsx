import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SystemLogo } from "../common/SystemLogo";
import { SettingsIcon } from "../common/Icons";

export function Sidebar() {
  const systems = useAppStore((s) => s.systems);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const selectSystem = useAppStore((s) => s.selectSystem);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const loading = useAppStore((s) => s.loading);
  const scanProgress = useAppStore((s) => s.scanProgress);
  const { t } = useTranslation();

  const isScanning = loading && scanProgress !== null;

  return (
    <aside className="w-64 bg-zinc-900 border-r border-zinc-700 flex flex-col h-full">
      <div className="p-4 border-b border-zinc-700">
        <h1 className="text-lg font-bold text-zinc-100">{t("sidebar.title")}</h1>
        <p className="text-xs text-zinc-500">{t("sidebar.subtitle")}</p>
      </div>

      <nav className="flex-1 overflow-y-auto py-2">
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
          systems.map((system) => (
            <button
              key={system.path}
              onClick={() => {
                selectSystem(system);
                setCurrentView("browser");
              }}
              className={`relative w-full text-left px-3 py-2 flex flex-col items-center gap-1 transition-colors ${
                selectedSystem?.path === system.path
                  ? "bg-indigo-600/20 text-indigo-400 border-r-2 border-indigo-500"
                  : "text-zinc-300 hover:bg-zinc-800"
              }`}
            >
              {/* Badge con número de juegos/roms */}
              {(system.has_gamelist ? system.game_count > 0 : system.rom_count > 0) && (
                <span className="absolute top-1.5 right-2 text-[10px] text-zinc-500 bg-zinc-800 px-1.5 py-0.5 rounded-full">
                  {system.has_gamelist ? system.game_count : system.rom_count}
                </span>
              )}
              <SystemLogo name={system.name} width={94} maxHeight={40} className="shrink-0 py-1" />
              <span className="text-xs block truncate w-full text-center pb-3" title={system.full_name}>
                {system.full_name}
              </span>
            </button>
          ))
        )}
      </nav>

      <div className="border-t border-zinc-700 h-12 shrink-0 flex items-center px-3">
        <button
          onClick={() => setCurrentView("settings")}
          className="w-full text-left px-3 py-1 text-sm text-zinc-400 hover:bg-zinc-800 rounded transition-colors flex items-center gap-2"
        >
          <SettingsIcon size={16} />
          {t("sidebar.settings")}
        </button>
      </div>
    </aside>
  );
}
