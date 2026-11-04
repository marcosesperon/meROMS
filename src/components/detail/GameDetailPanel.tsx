import { useState, useEffect, useCallback, useMemo } from "react";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { GameForm } from "./GameForm";
import { ImageManager } from "./ImageManager";
import { saveGamelist, removeGame, scrapeGame } from "../../commands";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { CloseIcon } from "../common/Icons";
import { RenameGameModal } from "../common/RenameGameModal";
import { NormalizeMediaModal } from "../common/NormalizeMediaModal";
import { UnusedMediaModal } from "../common/UnusedMediaModal";

// Per-game tools (the same ones the system toolbar has, limited to this game)
type GameTool = "rename" | "normalize" | "clean";

interface GameDetailPanelProps {
  isNewGame?: boolean;
  onClose: () => void;
}

export function GameDetailPanel({ isNewGame, onClose }: GameDetailPanelProps) {
  const selectedGame = useAppStore((s) => s.selectedGame);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const games = useAppStore((s) => s.games);
  const folders = useAppStore((s) => s.folders);
  const provider = useAppStore((s) => s.provider);
  const setGames = useAppStore((s) => s.setGames);
  const selectGame = useAppStore((s) => s.selectGame);
  const loadGames = useAppStore((s) => s.loadGames);
  const { t } = useTranslation();

  const initialGame = useMemo(
    () => (isNewGame ? { path: "" } : selectedGame ? { ...selectedGame } : { path: "" }),
    // Only compute once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
    []
  );

  const [editedGame, setEditedGame] = useState<Game>(initialGame);
  const [saving, setSaving] = useState(false);
  const [scraping, setScraping] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [showUnsavedConfirm, setShowUnsavedConfirm] = useState(false);
  const [activeTool, setActiveTool] = useState<GameTool | null>(null);
  // Tool waiting for the "unsaved changes" answer
  const [pendingTool, setPendingTool] = useState<GameTool | null>(null);

  useEffect(() => {
    if (!isNewGame && selectedGame) {
      setEditedGame({ ...selectedGame });
    }
  }, [selectedGame, isNewGame]);

  const hasChanges = useCallback((): boolean => {
    const original = isNewGame ? { path: "" } : selectedGame || { path: "" };
    const keys = new Set([
      ...Object.keys(original),
      ...Object.keys(editedGame),
    ]) as Set<keyof Game>;
    for (const key of keys) {
      const a = original[key] ?? "";
      const b = editedGame[key] ?? "";
      if (a !== b) return true;
    }
    return false;
  }, [editedGame, selectedGame, isNewGame]);

  const handleClose = useCallback(() => {
    if (hasChanges()) {
      setShowUnsavedConfirm(true);
    } else {
      onClose();
    }
  }, [hasChanges, onClose]);

  function handleFieldChange(field: keyof Game, value: string) {
    setEditedGame((prev) => ({ ...prev, [field]: value || undefined }));
  }

  /** Saves the game without closing the panel. Returns false if it failed. */
  async function persist(): Promise<boolean> {
    if (!selectedSystem) return false;
    setSaving(true);
    try {
      let updatedGames: Game[];
      if (isNewGame) {
        updatedGames = [...games, editedGame];
      } else {
        updatedGames = games.map((g) =>
          g.path === selectedGame?.path ? editedGame : g
        );
      }
      await saveGamelist(selectedSystem.path, { games: updatedGames, folders: folders || [], provider });
      setGames(updatedGames);
      if (!isNewGame) selectGame(editedGame);
      return true;
    } catch (e) {
      console.error("Failed to save:", e);
      alert(t("detail.saveFailed", { error: String(e) }));
      return false;
    } finally {
      setSaving(false);
    }
  }

  async function handleSave() {
    if (await persist()) onClose();
  }

  // The tools write the gamelist themselves: unsaved edits must be saved first
  function runTool(tool: GameTool) {
    if (hasChanges()) setPendingTool(tool);
    else setActiveTool(tool);
  }

  async function saveAndRunTool() {
    const tool = pendingTool;
    setPendingTool(null);
    if (tool && (await persist())) setActiveTool(tool);
  }

  /** After a tool: reload the system list and show this game again (maybe with a new path) */
  async function refreshGame(gamePath: string) {
    if (!selectedSystem) return;
    await loadGames(selectedSystem.path);
    const fresh = useAppStore.getState().games.find((g) => g.path === gamePath);
    selectGame(fresh ?? null);
  }

  async function handleScrape() {
    if (!selectedSystem || !editedGame.path) return;
    setScraping(true);
    try {
      const result = await scrapeGame(selectedSystem.path, editedGame.path);
      if (result.found) {
        setEditedGame(result.game);
      } else {
        alert(t("detail.scrapeNotFound"));
      }
    } catch (e) {
      console.error("Scrape failed:", e);
      alert(t("detail.scrapeFailed", { error: String(e) }));
    } finally {
      setScraping(false);
    }
  }

  async function handleDelete() {
    if (!selectedSystem || !selectedGame) return;
    try {
      const result = await removeGame(selectedSystem.path, selectedGame.path);
      setGames(result.games);
      selectGame(null);
      onClose();
    } catch (e) {
      console.error("Failed to delete:", e);
      alert(t("detail.deleteFailed", { error: String(e) }));
    }
  }

  return (
    <>
      <div className="fixed inset-0 bg-black/40 z-40" onClick={handleClose} />
      <div className="fixed top-0 right-0 bottom-0 w-[480px] bg-zinc-900 border-l border-zinc-700 flex flex-col overflow-hidden z-50 shadow-2xl">
        <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-700">
          <h3 className="text-sm font-semibold text-zinc-200">
            {isNewGame ? t("detail.addGame") : t("detail.editGame")}
          </h3>
          <div className="flex items-center gap-2">
            {!isNewGame && (
              <>
                <button
                  onClick={handleScrape}
                  disabled={scraping}
                  className="px-3 py-1.5 text-xs bg-emerald-600/20 text-emerald-400 rounded hover:bg-emerald-600/30 transition-colors disabled:opacity-50"
                >
                  {scraping ? t("detail.scraping") : t("detail.scrape")}
                </button>
                <button
                  onClick={() => setShowDeleteConfirm(true)}
                  className="px-3 py-1.5 text-xs bg-red-600/20 text-red-400 rounded hover:bg-red-600/30 transition-colors"
                >
                  {t("detail.delete")}
                </button>
              </>
            )}
            <button
              onClick={handleClose}
              className="text-zinc-400 hover:text-zinc-200 p-1"
            >
              <CloseIcon size={16} />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-6">
          {selectedSystem && (
            <ImageManager
              game={editedGame}
              systemPath={selectedSystem.path}
              onUpdate={handleFieldChange}
              onNormalize={!isNewGame ? () => runTool("normalize") : undefined}
              onClean={!isNewGame ? () => runTool("clean") : undefined}
            />
          )}
          <GameForm
            game={editedGame}
            onChange={handleFieldChange}
            onRenameRom={!isNewGame ? () => runTool("rename") : undefined}
          />
        </div>

        {/* Sin «Cancelar»: para salir sin guardar ya está la X de arriba */}
        <div className="border-t border-zinc-700 h-12 shrink-0 px-4 flex items-center justify-end gap-3">
          <button
            onClick={handleSave}
            disabled={saving || !editedGame.path}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {saving ? t("detail.saving") : t("detail.save")}
          </button>
        </div>
      </div>

      <ConfirmDialog
        open={showDeleteConfirm}
        title={t("detail.deleteTitle")}
        message={t("detail.deleteConfirm", { name: editedGame.name || editedGame.path })}
        onConfirm={handleDelete}
        onCancel={() => setShowDeleteConfirm(false)}
      />

      {/* Unsaved changes before running a tool */}
      {pendingTool && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]">
          <div className="bg-zinc-800 border border-zinc-600 rounded-lg p-6 max-w-md w-full mx-4">
            <h3 className="text-lg font-semibold text-zinc-100 mb-2">
              {t("detail.unsavedTitle")}
            </h3>
            <p className="text-zinc-400 mb-6">
              {t("gameTools.unsavedMessage")}
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setPendingTool(null)}
                className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("detail.cancel")}
              </button>
              <button
                onClick={saveAndRunTool}
                className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
              >
                {t("gameTools.saveAndContinue")}
              </button>
            </div>
          </div>
        </div>
      )}

      {selectedSystem && selectedGame && (
        <>
          <RenameGameModal
            open={activeTool === "rename"}
            systemPath={selectedSystem.path}
            gamePath={selectedGame.path}
            onClose={() => setActiveTool(null)}
            onDone={refreshGame}
          />
          <NormalizeMediaModal
            open={activeTool === "normalize"}
            systemPath={selectedSystem.path}
            gamePath={selectedGame.path}
            gameName={selectedGame.name}
            onClose={() => setActiveTool(null)}
            onDone={() => refreshGame(selectedGame.path)}
          />
          <UnusedMediaModal
            open={activeTool === "clean"}
            systemPath={selectedSystem.path}
            gamePath={selectedGame.path}
            gameName={selectedGame.name}
            onClose={() => setActiveTool(null)}
          />
        </>
      )}

      {/* Unsaved changes dialog */}
      {showUnsavedConfirm && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]">
          <div className="bg-zinc-800 border border-zinc-600 rounded-lg p-6 max-w-md w-full mx-4">
            <h3 className="text-lg font-semibold text-zinc-100 mb-2">
              {t("detail.unsavedTitle")}
            </h3>
            <p className="text-zinc-400 mb-6">
              {t("detail.unsavedMessage")}
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setShowUnsavedConfirm(false)}
                className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
              >
                {t("detail.cancel")}
              </button>
              <button
                onClick={() => {
                  setShowUnsavedConfirm(false);
                  onClose();
                }}
                className="px-4 py-2 text-sm bg-red-600 text-white rounded hover:bg-red-500 transition-colors"
              >
                {t("detail.discard")}
              </button>
              <button
                onClick={() => {
                  setShowUnsavedConfirm(false);
                  handleSave();
                }}
                className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
              >
                {t("detail.save")}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
