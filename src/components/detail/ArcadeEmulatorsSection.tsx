import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import type { Game } from "../../types";
import { applyEmulators, arcadeCheck, type ArcadeGame, type ArcadeReport } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import { showMessage } from "../../utils/dialogs";
import { reportWithUndo } from "../../utils/undo";
import { CORE_NAMES, STATE_COLOR, STATE_MARK, checkProblems } from "../../utils/arcadeSets";

/**
 * Ficha del juego (pestaña Metadatos), sistemas arcade: cómo está su set en cada núcleo con DAT,
 * con cuál se juega ahora y, si no le vale, el que se propone (con un botón para usarlo).
 */
export function ArcadeEmulatorsSection({ game, systemPath }: { game: Game; systemPath: string }) {
  const { t } = useTranslation();
  // undefined = comprobando
  const [report, setReport] = useState<ArcadeReport | null | undefined>(undefined);
  const [saving, setSaving] = useState(false);

  async function load() {
    setReport(await arcadeCheck(systemPath, [game.path]).catch(() => null));
  }

  useEffect(() => {
    setReport(undefined);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [systemPath, game.path, game.altemulator]);

  if (report === undefined) return <p className="text-sm text-zinc-500">…</p>;
  if (!report || report.cores.length === 0) return <p className="text-sm text-zinc-500">{t("arcade.metaNoDats")}</p>;
  const info: ArcadeGame | undefined = report.games[0];
  if (!info) return <p className="text-sm text-zinc-500">{t("arcade.metaNotASet")}</p>;

  /** Cambia el emulador del juego (null = el del sistema), en el gamelist y en memoria */
  async function setEmulator(label: string | null) {
    setSaving(true);
    try {
      await applyEmulators(systemPath, [{ path: game.path, label }]);
      const { games, selectedGame } = useAppStore.getState();
      const patch = (g: Game) => (g.path === game.path ? { ...g, altemulator: label ?? undefined } : g);
      useAppStore.setState({ games: games.map(patch), selectedGame: selectedGame ? patch(selectedGame) : selectedGame });
      await reportWithUndo(label ? t("arcade.metaUsed", { emulator: label }) : t("arcade.metaReset"));
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-2">
      <div className="space-y-1">
        {report.cores.map((id) => {
          const c = info.checks[id];
          if (!c) return null;
          const problems = checkProblems(c, t);
          return (
            <div key={id} className="flex items-start gap-2 text-sm">
              <span className={`w-4 text-center font-semibold ${STATE_COLOR[c.state]}`}>{STATE_MARK[c.state]}</span>
              <div className="min-w-0">
                <p className="text-zinc-200">
                  {CORE_NAMES[id]}
                  {info.current_core === id && <span className="ml-2 text-[11px] px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-300">{t("arcade.metaCurrent")}</span>}
                  <span className="text-zinc-500 text-xs"> · {t(`arcade.state_${c.state}` as TranslationKeys)}</span>
                  {c.driver && c.state !== "absent" && <span className="text-zinc-500 text-xs"> · {t(`arcade.driver_${c.driver}` as TranslationKeys)}</span>}
                </p>
                {problems.map((line) => (
                  <p key={line} className="text-xs text-amber-300">
                    {line}
                  </p>
                ))}
              </div>
            </div>
          );
        })}
      </div>
      <p className="text-xs text-zinc-400">
        {t("arcade.metaPlaysWith", { emulator: info.current ?? "—" })}
        {info.own_emulator && <span className="text-zinc-500"> ({t("arcade.ownEmulator")})</span>}
      </p>
      <div className="flex flex-wrap gap-2">
        {info.proposal && (
          <button onClick={() => setEmulator(info.proposal)} disabled={saving} className="px-3 py-1.5 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 disabled:opacity-50">
            {t("arcade.metaUse", { emulator: info.proposal })}
          </button>
        )}
        {info.own_emulator && (
          <button onClick={() => setEmulator(null)} disabled={saving} className="px-3 py-1.5 text-xs bg-zinc-700 text-zinc-200 rounded hover:bg-zinc-600 disabled:opacity-50">
            {t("arcade.metaBackToSystem")}
          </button>
        )}
      </div>
    </div>
  );
}
