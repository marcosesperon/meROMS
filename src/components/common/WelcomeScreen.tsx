import { useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "../../i18n";
import { useAppStore } from "../../store/useAppStore";
import { getActiveProfile } from "../../utils/profiles";
// El icono de la app tal cual (el mismo SVG del que salen los iconos), sin copias que mantener
import appIcon from "../../../src-tauri/icons/source/meroms.svg";
import { LibraryStats } from "../stats/LibraryStats";
import { HomeHero } from "../stats/HomeHero";
import { SystemTiles } from "../stats/SystemTiles";
import { RaProfileCard } from "../stats/RaProfileCard";
import { ContinuePlaying } from "../stats/ContinuePlaying";
import { continuePlaying, libraryStats, type ContinueGame, type LibraryStats as Stats } from "../../commands";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "./TaskProgress";

/**
 * Pantalla de inicio del centro cuando no hay sistema elegido. Con sistemas que enseñar: icono,
 * nombre y versión en pequeño y, debajo, las estadísticas de la biblioteca. Si no: la
 * presentación grande con qué hacer (enseñar los ocultos o revisar el perfil).
 */
export function WelcomeScreen() {
  const { t } = useTranslation();
  const systems = useAppStore((s) => s.systems);
  // Escaneando los sistemas (no la carga de la lista de juegos)
  const loading = useAppStore((s) => s.systemsLoading);
  // Progreso de la lectura de sistemas del perfil (se enseña aquí, con sitio)
  const scanProgress = useAppStore((s) => s.scanProgress);
  const onlyWithGames = useAppStore((s) => s.onlySystemsWithGames);
  const toggleOnlyWithGames = useAppStore((s) => s.toggleOnlySystemsWithGames);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const romFolder = useAppStore((s) => getActiveProfile(s.settings)?.rom_root_path);
  const profileName = useAppStore((s) => getActiveProfile(s.settings)?.name ?? "");
  // La carpeta del perfil no existe ahora (SD o disco sin conectar): App la vuelve a mirar sola
  const unavailable = useAppStore((s) => s.profileUnavailable);
  // La versión se lee de la propia app (tauri.conf.json)
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  // Mismo criterio que la barra lateral para «sistemas con juegos»
  const withGames = systems.filter((s) => (s.has_gamelist ? s.game_count : s.rom_count) > 0);
  const allHidden = onlyWithGames && systems.length > 0 && withGames.length === 0;
  const button = "mt-4 px-4 py-2 text-sm bg-indigo-600 text-white rounded-md hover:bg-indigo-500 transition-colors";

  // Estadísticas de la biblioteca (los sistemas con gamelist): una lectura al entrar
  const showHome = !loading && systems.length > 0 && !allHidden;
  const paths = useMemo(() => systems.filter((s) => s.has_gamelist).map((s) => s.path), [systems]);
  const statsTask = useTask();
  const [stats, setStats] = useState<Stats | null>(null);
  useEffect(() => {
    if (!showHome || paths.length === 0) return;
    statsTask.run((task) => libraryStats(paths, task)).then((r) => r && setStats(r)).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHome, paths]);
  // «Seguir jugando» va aparte: es rápido y no espera a las estadísticas
  const [continueGames, setContinueGames] = useState<ContinueGame[]>([]);
  useEffect(() => {
    if (!showHome || paths.length === 0) return;
    continuePlaying(paths).then(setContinueGames).catch(() => {});
  }, [showHome, paths]);
  const fullName = (path: string) => systems.find((s) => s.path === path)?.full_name ?? path;

  // Hay biblioteca que enseñar: portada (cabecera con mosaico y cifras), sistemas y estadísticas
  if (showHome) {
    return (
      <div className="flex-1 overflow-y-auto select-none">
        <div className="max-w-6xl mx-auto px-6 py-6 space-y-7">
          <HomeHero stats={stats} version={version} />
          <ContinuePlaying games={continueGames} />
          <SystemTiles stats={stats?.systems ?? null} />
          <RaProfileCard />
          {stats ? (
            <LibraryStats stats={stats} paths={paths} fullName={fullName} />
          ) : paths.length > 0 ? (
            <div className="max-w-md mx-auto">
              <TaskProgress progress={statsTask.progress} label={t("stats.loading")} />
            </div>
          ) : null}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-8 text-center select-none">
      <img src={appIcon} alt="" className="w-40 h-40 drop-shadow-[0_8px_24px_rgba(0,0,0,0.5)]" />
      <h1 className="mt-4 text-3xl font-bold text-zinc-100">
        meROMS {version && <span className="text-lg font-normal text-zinc-500">v{version}</span>}
      </h1>
      <p className="mt-2 max-w-md text-sm text-zinc-400">{t("about.description")}</p>

      <div className="mt-8 max-w-md">
        {unavailable ? (
          // Perfil en una SD o un disco que no está conectado: se cargará solo al conectarlo
          <>
            <p className="text-sm text-amber-300">{t("welcome.unavailable", { name: profileName })}</p>
            {romFolder && <p className="mt-1 text-xs text-zinc-500 font-mono break-all">{romFolder}</p>}
            <p className="mt-2 text-xs text-zinc-400">{t("welcome.unavailableHelp")}</p>
            <button onClick={() => setCurrentView("settings")} className={button}>
              {t("welcome.openSettings")}
            </button>
          </>
        ) : loading ? (
          // Leyendo los sistemas del perfil: cuántos van y cuál está leyendo
          <div className="w-80 max-w-full">
            <TaskProgress progress={scanProgress} label={t("sidebar.scanning")} compact />
          </div>
        ) : systems.length === 0 ? (
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
