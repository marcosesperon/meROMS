import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { SystemLogo, hasSystemLogo } from "../common/SystemLogo";
import { SettingsIcon } from "../common/Icons";
import { getActiveProfile } from "../../utils/profiles";

export function Sidebar() {
  const systems = useAppStore((s) => s.systems);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const selectSystem = useAppStore((s) => s.selectSystem);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const loading = useAppStore((s) => s.loading);
  const scanProgress = useAppStore((s) => s.scanProgress);
  const settings = useAppStore((s) => s.settings);
  const setActiveProfile = useAppStore((s) => s.setActiveProfile);
  const pendingStructure = useAppStore((s) => s.pendingStructure);
  const { t } = useTranslation();

  const profiles = settings.profiles ?? [];
  const activeProfile = getActiveProfile(settings);

  const isScanning = loading && scanProgress !== null;

  return (
    <aside className="w-48 bg-zinc-950 border-r border-zinc-800 flex flex-col h-full">
      <div className="p-4 border-b border-zinc-800">
        <h1 className="text-lg font-bold tracking-tight text-zinc-100">{t("sidebar.title")}</h1>
        <p className="text-xs text-zinc-500">{t("sidebar.subtitle")}</p>
        {profiles.length > 0 && (
          <label className="block mt-3">
            <span className="text-[10px] uppercase tracking-wide text-zinc-500">{t("sidebar.profile")}</span>
            <select
              value={activeProfile?.id ?? ""}
              onChange={(e) => setActiveProfile(e.target.value)}
              disabled={loading}
              className="mt-1 w-full bg-zinc-800 border border-zinc-700 rounded-md px-2 py-1.5 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500 disabled:opacity-50"
            >
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.kind === "esde" ? "ES-DE" : t("profiles.kindClassic")})
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <p className="px-4 pt-4 pb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
        {t("sidebar.systems")}
      </p>
      <nav className="flex-1 overflow-y-auto pb-2">
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
                className={`group relative w-full text-left pl-4 pr-3 py-1.5 flex items-center gap-3 transition-colors ${
                  selected ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"
                }`}
              >
                {/* Franja violeta a la izquierda del sistema elegido (como GOG) */}
                {selected && <span className="absolute left-0 top-1 bottom-1 w-[3px] rounded-r bg-indigo-500" />}
                {/* Solo el logo (el nombre completo sale al pasar el ratón); si no hay logo,
                    el nombre corto del sistema (el de su carpeta) */}
                <span className="flex-1 min-w-0 h-7 flex items-center">
                  {hasSystemLogo(system.name) ? (
                    <SystemLogo
                      name={system.name}
                      width={56}
                      maxHeight={22}
                      // Los no elegidos, algo apagados (como el texto gris de antes)
                      className={selected ? "" : "opacity-60 group-hover:opacity-100 transition-opacity"}
                    />
                  ) : (
                    <span className="text-sm font-semibold uppercase tracking-wide truncate">{system.name}</span>
                  )}
                </span>
                {/* Marca: este sistema está en la otra estructura y se puede migrar */}
                {pendingStructure?.systems.includes(system.name) && (
                  <span
                    className="w-2 h-2 shrink-0 rounded-full bg-amber-400"
                    title={t("structure.mark", {
                      kind: pendingStructure.source.kind === "esde" ? "ES-DE" : t("profiles.kindClassic"),
                    })}
                  />
                )}
                {count > 0 && (
                  <span
                    className={`text-[10px] shrink-0 px-1.5 py-0.5 rounded-full ${
                      selected ? "bg-indigo-600 text-white" : "bg-zinc-800 text-zinc-500"
                    }`}
                  >
                    {count}
                  </span>
                )}
              </button>
            );
          })
        )}
      </nav>

      <div className="border-t border-zinc-800 h-12 shrink-0 flex items-center px-3">
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
