import { useEffect, useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { TranslationKeys } from "../../i18n/en";
import { applyEmulators } from "../../commands";
import { showMessage } from "../../utils/dialogs";
import { CORE_IDS, CORE_NAMES, STATE_COLOR, STATE_MARK, checkProblems } from "../../utils/arcadeSets";

/**
 * Arcade, al pulsar Jugar: el set no está completo para el emulador que se va a usar. Dice qué
 * falta y, si otro núcleo lo tiene completo: jugar con ese esta vez, jugar con ese y recordarlo
 * (escribe su `<altemulator>`), o jugar igualmente.
 */
export function ArcadePlayWarning() {
  const { t } = useTranslation();
  const warning = useAppStore((s) => s.arcadeWarning);
  const setWarning = useAppStore((s) => s.setArcadeWarning);
  const [saving, setSaving] = useState(false);
  const close = () => setWarning(null);

  // Esc (o B en el mando) la cierra
  useEffect(() => {
    if (!warning) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setWarning(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [warning, setWarning]);

  if (!warning) return null;
  const { game, check } = warning;
  const current = check.current_core ? check.checks[check.current_core] : undefined;
  const proposal = check.proposal;

  const play = (emulator?: string) => {
    close();
    useAppStore.getState().playGame(game, { emulator, skipArcadeCheck: true });
  };

  /** Lo deja escrito en el gamelist (con Deshacer) y juega con él */
  async function rememberAndPlay() {
    if (!proposal) return;
    const { selectedSystem, games, selectedGame, refreshUndo } = useAppStore.getState();
    if (!selectedSystem) return;
    setSaving(true);
    try {
      await applyEmulators(selectedSystem.path, [{ path: game.path, label: proposal }]);
      // También en memoria: si no, guardar el juego en el editor lo quitaría
      const updated = { ...game, altemulator: proposal };
      useAppStore.setState({
        games: games.map((g) => (g.path === game.path ? { ...g, altemulator: proposal } : g)),
        selectedGame: selectedGame?.path === game.path ? { ...selectedGame, altemulator: proposal } : selectedGame,
      });
      refreshUndo();
      close();
      useAppStore.getState().playGame(updated, { skipArcadeCheck: true });
    } catch (e) {
      await showMessage(String(e), "error");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onMouseDown={close}>
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[560px] max-w-[calc(100vw-2rem)] flex flex-col mx-4" onMouseDown={(e) => e.stopPropagation()}>
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("arcade.warnTitle", { name: game.name || game.path })}</h3>
          <p className="text-xs text-zinc-400 mt-1">{t("arcade.warnEmulator", { emulator: check.current ?? "" })}</p>
        </div>
        <div className="px-5 py-4 space-y-3 text-sm text-zinc-300">
          {current && (
            <ul className="space-y-0.5">
              {checkProblems(current, t).map((line) => (
                <li key={line} className="text-amber-300">
                  {line}
                </li>
              ))}
            </ul>
          )}
          {/* Cómo está en cada núcleo con DAT */}
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            {CORE_IDS.filter((id) => check.checks[id]).map((id) => {
              const c = check.checks[id]!;
              return (
                <p key={id} className="flex items-center gap-2">
                  <span className={`w-4 text-center font-semibold ${STATE_COLOR[c.state]}`}>{STATE_MARK[c.state]}</span>
                  <span className="text-zinc-300">{CORE_NAMES[id]}</span>
                  <span className="text-zinc-500 truncate">{t(`arcade.state_${c.state}` as TranslationKeys)}</span>
                </p>
              );
            })}
          </div>
          <p className={proposal ? "text-emerald-300" : "text-zinc-400"}>{proposal ? t("arcade.warnWorksWith", { emulator: proposal }) : t("arcade.warnNoCore")}</p>
        </div>
        <div className="border-t border-zinc-700 px-5 py-3 flex flex-wrap justify-end gap-2">
          <button onClick={close} className="px-4 py-2 text-sm text-zinc-300 rounded border border-zinc-600 hover:bg-zinc-700">
            {t("confirm.cancel")}
          </button>
          <button onClick={() => play()} className="px-4 py-2 text-sm text-zinc-300 rounded border border-zinc-600 hover:bg-zinc-700">
            {t("arcade.playAnyway")}
          </button>
          {proposal && (
            <>
              <button onClick={rememberAndPlay} disabled={saving} className="px-4 py-2 text-sm bg-zinc-700 text-zinc-100 rounded hover:bg-zinc-600 disabled:opacity-50">
                {t("arcade.playWithRemember", { emulator: proposal })}
              </button>
              <button autoFocus onClick={() => play(proposal)} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500">
                {t("arcade.playWith", { emulator: proposal })}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
