import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { SystemInfo } from "../../types";
import { appExecutable, launchOptions, resolveLaunch, setSystemEmulator, type LaunchOptions, type LaunchPlan } from "../../commands";
import { getSystemView, setSystemView } from "../../utils/systemView";

/** Valor del desplegable para «Comando propio» */
const CUSTOM = "__custom";

/**
 * Emulador de un sistema (en su ventanita de vista): el predeterminado de ES-DE, otro de sus
 * emuladores (se guarda en el gamelist, `<alternativeEmulator>`, donde lo lee ES-DE) o un
 * comando propio (en el perfil, con las variables de ES-DE). «Probar» enseña el comando final
 * con el juego seleccionado, sin lanzar nada.
 */
export function EmulatorSection({ system }: { system: SystemInfo }) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const firstGame = useAppStore((s) => s.games[0]);
  const [options, setOptions] = useState<LaunchOptions | null>(null);
  const [test, setTest] = useState<LaunchPlan | null>(null);

  const reload = () => launchOptions(system.path).then(setOptions).catch(() => setOptions(null));
  useEffect(() => {
    reload();
    setTest(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [system.path]);

  const command = getSystemView(settings, system.name).launch_command;
  const isCustom = command !== undefined || options?.known === false;
  const value = isCustom ? CUSTOM : options?.current ?? "";

  const choose = async (next: string) => {
    setTest(null);
    if (next === CUSTOM) {
      setSystemView(system.name, { launch_command: command ?? "" });
      return;
    }
    setSystemView(system.name, { launch_command: undefined });
    await setSystemEmulator(system.path, next || null);
    reload();
  };

  // Elegir el ejecutable del emulador: el comando queda «"ruta" %ROM%» para retocarlo
  const pickEmulator = async () => {
    const picked = await open({ multiple: false, directory: false });
    if (typeof picked !== "string") return;
    const exe = await appExecutable(picked);
    setSystemView(system.name, { launch_command: `"${exe}" %ROM%` });
  };

  const runTest = async () => {
    const game = selectedGame ?? firstGame;
    if (!game) return;
    setTest(await resolveLaunch(system.path, game.path));
  };

  const defaultLabel = options?.options[0]?.label;
  const notFound = (available: boolean) => (available ? "" : ` — ${t("play.notFound")}`);

  return (
    <div className="space-y-2">
      <span className="text-xs text-zinc-400">{t("play.emulator")}</span>
      <select
        value={value}
        onChange={(e) => choose(e.target.value)}
        className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-indigo-500"
      >
        {options?.known && (
          <>
            <option value="">
              {t("play.emuDefault", { name: defaultLabel ?? "" })}
              {notFound(options.options[0]?.available ?? false)}
            </option>
            {options.options.slice(1).map((o) => (
              <option key={o.label} value={o.label}>
                {o.label}
                {notFound(o.available)}
              </option>
            ))}
          </>
        )}
        <option value={CUSTOM}>{t("play.emuCustom")}</option>
      </select>
      {options?.known === false && <p className="text-[11px] text-amber-300/90">{t("play.unknownSystem")}</p>}

      {isCustom && (
        <div className="space-y-2">
          <input
            value={command ?? ""}
            onChange={(e) => setSystemView(system.name, { launch_command: e.target.value })}
            placeholder={t("play.customPlaceholder")}
            spellCheck={false}
            className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 font-mono text-xs text-zinc-200 focus:outline-none focus:border-indigo-500"
          />
          <p className="text-[11px] text-zinc-500">{t("play.customHelp")}</p>
        </div>
      )}

      <div className="flex gap-2">
        {isCustom && (
          <button onClick={pickEmulator} className="px-3 py-1.5 text-xs text-zinc-200 rounded border border-zinc-600 hover:bg-zinc-700">
            {t("play.pickEmulator")}
          </button>
        )}
        <button
          onClick={runTest}
          disabled={!selectedGame && !firstGame}
          className="px-3 py-1.5 text-xs text-zinc-200 rounded border border-zinc-600 hover:bg-zinc-700 disabled:opacity-40"
        >
          {t("play.test")}
        </button>
      </div>

      {/* Resultado de «Probar»: el comando final o lo que falta */}
      {test && (
        <div data-launch-test className="rounded bg-zinc-900 border border-zinc-700 px-3 py-2 text-[11px] space-y-1">
          {test.command ? (
            <>
              <p className="text-emerald-400">{t("play.testOk", { name: (selectedGame ?? firstGame)?.name ?? "" })}</p>
              <p className="font-mono text-zinc-300 break-all">{test.command}</p>
            </>
          ) : test.no_config ? (
            <p className="text-amber-300">{t("play.noConfig", { system: system.name })}</p>
          ) : test.missing ? (
            <p className="text-amber-300">
              {t(test.missing.kind === "core" ? "play.missingCore" : "play.missingEmulator", { name: test.missing.name })}
            </p>
          ) : null}
        </div>
      )}
    </div>
  );
}
