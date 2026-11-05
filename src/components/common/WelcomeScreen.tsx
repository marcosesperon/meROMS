import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";
// El icono de la app tal cual (el mismo SVG del que salen los iconos), sin copias que mantener
import appIcon from "../../../src-tauri/icons/source/meroms.svg";

/**
 * Pantalla de presentación del centro cuando no hay sistema elegido: icono, nombre, versión y,
 * debajo, qué hacer según el caso (elegir un sistema, enseñar los ocultos o revisar el perfil).
 */
export function WelcomeScreen() {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  const loading = useAppStore((s) => s.loading);
  const onlyWithGames = useAppStore((s) => s.onlySystemsWithGames);
  const toggleOnlyWithGames = useAppStore((s) => s.toggleOnlySystemsWithGames);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const romFolder = useAppStore((s) => getActiveProfile(s.settings)?.rom_root_path);
  // La versión se lee de la propia app (tauri.conf.json)
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  // Mismo criterio que la barra lateral para «sistemas con juegos»
  const withGames = systems.filter((s) => (s.has_gamelist ? s.game_count : s.rom_count) > 0);
  const allHidden = onlyWithGames && systems.length > 0 && withGames.length === 0;
  const button = "mt-4 px-4 py-2 text-sm bg-indigo-600 text-white rounded-md hover:bg-indigo-500 transition-colors";

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 text-center select-none">
      <img src={appIcon} alt="" className="w-40 h-40 drop-shadow-[0_8px_24px_rgba(0,0,0,0.5)]" />
      <h1 className="mt-4 text-3xl font-bold text-zinc-100">
        meROMS {version && <span className="text-lg font-normal text-zinc-500">v{version}</span>}
      </h1>
      <p className="mt-2 max-w-md text-sm text-zinc-400">{t("about.description")}</p>

      <div className="mt-8 max-w-md">
        {loading ? null : systems.length === 0 ? (
          // La carpeta del perfil no tiene sistemas (o no hay perfil)
          <>
            <p className="text-sm text-zinc-300">{t("welcome.noSystems")}</p>
            {romFolder && <p className="mt-1 text-xs text-zinc-500 font-mono break-all">{romFolder}</p>}
            <button onClick={() => setCurrentView("settings")} className={button}>
              {t("welcome.openSettings")}
            </button>
          </>
        ) : allHidden ? (
          // Todos ocultos por el interruptor «solo sistemas con juegos»
          <>
            <p className="text-sm text-zinc-300">{t("welcome.allHidden", { count: systems.length })}</p>
            <button onClick={toggleOnlyWithGames} className={button}>
              {t("welcome.showAll")}
            </button>
          </>
        ) : (
          <p className="text-sm text-zinc-300">{t("welcome.pickSystem")}</p>
        )}
      </div>
    </div>
  );
}
