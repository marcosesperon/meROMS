import { useState } from "react";
import { useTask } from "../../hooks/useTask";
import { TaskProgress } from "../common/TaskProgress";
import { useAppStore } from "../../store/useAppStore";
import { useGames } from "../../hooks/useGames";
import { useFilteredGames } from "../../hooks/useFilteredGames";
import { useTranslation } from "../../i18n";
import { generateGamelist } from "../../commands";
import { GameTable } from "./GameTable";
import { GameGrid } from "./GameGrid";
import { EmptyState } from "../common/EmptyState";
import { WelcomeScreen } from "../common/WelcomeScreen";
import { Header } from "../layout/Header";
import { FolderIcon } from "../common/Icons";
import { showMessage } from "../../utils/dialogs";

export function GameListView() {
  const viewMode = useAppStore((s) => s.viewMode);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const setGames = useAppStore((s) => s.setGames);
  const loadSystems = useAppStore((s) => s.loadSystems);
  const folders = useAppStore((s) => s.folders);
  const games = useAppStore((s) => s.games);
  const activeFolderPath = useAppStore((s) => s.activeFolderPath);
  const setActiveFolderPath = useAppStore((s) => s.setActiveFolderPath);
  const { loading } = useGames();
  const filteredGames = useFilteredGames();
  // Generar la gamelist (con progreso y Cancelar) y lo que avisa la carga de un sistema
  const generateTask = useTask();
  const generating = generateTask.running;
  const loadProgress = useAppStore((s) => s.loadProgress);
  const [foldersExpanded, setFoldersExpanded] = useState(true);
  const { t } = useTranslation();

  if (!selectedSystem) {
    return <WelcomeScreen />;
  }

  async function handleGenerate() {
    if (!selectedSystem) return;
    const systemPath = selectedSystem.path;
    try {
      const result = await generateTask.run((task) => generateGamelist(systemPath, task));
      // Cancelado: no se ha escrito nada
      if (!result) return;
      setGames(result.games);
      loadSystems();
    } catch (e) {
      console.error("Failed to generate gamelist:", e);
      await showMessage(t("generate.failed", { error: String(e) }), "error");
    }
  }

  const noGamelist = !selectedSystem.has_gamelist && selectedSystem.rom_count > 0;

  // Count games per folder
  function gamesInFolder(folderPath: string): number {
    return games.filter((g) => g.path.startsWith(folderPath)).length;
  }

  return (
    <>
      <Header />
      {loading || generating ? (
        <div className="flex-1 flex items-center justify-center">
          {generating ? (
            <TaskProgress
              progress={generateTask.progress}
              label={t("gameList.generating")}
              onCancel={generateTask.cancel}
              cancelling={generateTask.cancelling}
            />
          ) : (
            // Cargar un sistema no se cancela: es lo que se va a ver
            <TaskProgress progress={loadProgress} label={t("gameList.loading")} />
          )}
        </div>
      ) : games.length === 0 && noGamelist ? (
        <EmptyState
          icon="📂"
          title={t("gameList.noGamelist")}
          description={t("gameList.noGamelistDesc", { count: selectedSystem.rom_count })}
          action={{
            label: t("gameList.generateBtn"),
            onClick: handleGenerate,
          }}
        />
      ) : games.length === 0 ? (
        <EmptyState
          icon="📂"
          title={t("gameList.noGames")}
          description={t("gameList.noGamesDesc")}
        />
      ) : (
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Folder section */}
          {folders.length > 0 && (
            <div className="border-b border-zinc-700">
              <button
                onClick={() => setFoldersExpanded((v) => !v)}
                className="flex items-center gap-2 px-6 py-2 text-xs text-zinc-400 hover:text-zinc-200 transition-colors w-full text-left"
              >
                <span className={`transition-transform ${foldersExpanded ? "rotate-90" : ""}`}>
                  ▶
                </span>
                {t("filter.folders")} ({folders.length})
              </button>
              {foldersExpanded && (
                <div className="flex flex-wrap gap-2 px-6 pb-3">
                  {/* Show all (clear folder filter) */}
                  <button
                    onClick={() => setActiveFolderPath(null)}
                    className={`flex items-center gap-1.5 text-xs px-3 py-1.5 rounded border transition-colors ${
                      !activeFolderPath
                        ? "bg-indigo-600/20 border-indigo-500/50 text-indigo-300"
                        : "bg-zinc-700/50 border-zinc-600 text-zinc-400 hover:text-zinc-200"
                    }`}
                  >
                    {t("filter.all")}
                  </button>
                  {folders.map((folder) => (
                    <button
                      key={folder.path}
                      onClick={() =>
                        setActiveFolderPath(
                          activeFolderPath === folder.path ? null : folder.path
                        )
                      }
                      className={`flex items-center gap-1.5 text-xs px-3 py-1.5 rounded border transition-colors ${
                        activeFolderPath === folder.path
                          ? "bg-indigo-600/20 border-indigo-500/50 text-indigo-300"
                          : "bg-zinc-700/50 border-zinc-600 text-zinc-400 hover:text-zinc-200"
                      }`}
                    >
                      <FolderIcon size={12} />
                      {folder.name || folder.path}
                      <span className="text-zinc-500 ml-1">
                        {gamesInFolder(folder.path)}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Games */}
          {viewMode === "table" ? (
            <GameTable games={filteredGames} />
          ) : (
            <GameGrid games={filteredGames} />
          )}
        </div>
      )}
    </>
  );
}
