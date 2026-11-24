import { useAppStore } from "../../store/useAppStore";
import { useGameActions } from "../../hooks/useGameActions";
import { RenameModal } from "../common/RenameModal";
import { NormalizeMediaModal } from "../common/NormalizeMediaModal";
import { UnusedMediaModal } from "../common/UnusedMediaModal";
import { VersionsModal } from "../common/VersionsModal";
import { MoveGamesModal } from "../common/MoveGamesModal";
import { ScrapeSearchModal } from "../common/ScrapeSearchModal";
import { GameReviewModal } from "../detail/GameReviewModal";

/**
 * Las herramientas de un juego (o de los marcados) que abre el menú del botón derecho de la
 * lista, sin pasar por el editor: las mismas ventanas que usa el editor, con `gameTool` del
 * store diciendo cuál y para qué juegos. Al terminar se recarga la lista y se vuelve a
 * seleccionar el juego (con su ruta nueva si se renombró), como hace el editor.
 */
export function GameToolsHost() {
  const tool = useAppStore((s) => s.gameTool);
  const close = useAppStore((s) => s.closeGameTool);
  const openGameTool = useAppStore((s) => s.openGameTool);
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const game = useAppStore((s) => (tool?.paths.length === 1 ? s.games.find((g) => g.path === tool.paths[0]) ?? null : null));
  const { applyScraped } = useGameActions();

  if (!tool || !selectedSystem) return null;
  const systemPath = selectedSystem.path;

  /** Recargar y dejar seleccionado el juego (si sigue en la lista) */
  async function refresh(path: string | undefined) {
    const { loadGames, selectGame } = useAppStore.getState();
    await loadGames(systemPath);
    if (!path) return;
    const fresh = useAppStore.getState().games.find((g) => g.path === path);
    selectGame(fresh ?? null);
  }

  /** Mover y versiones pueden sacar juegos de la lista: recarga también la barra de sistemas */
  async function reloadAll() {
    const { loadSystems, setSelectedPaths } = useAppStore.getState();
    setSelectedPaths(new Set());
    await refresh(tool?.paths.length === 1 ? tool.paths[0] : undefined);
    await loadSystems();
  }

  switch (tool.id) {
    case "scrape":
      return game ? (
        <ScrapeSearchModal
          systemPath={systemPath}
          game={game}
          onClose={close}
          onApplied={async (scraped) => {
            close();
            await applyScraped(scraped);
          }}
        />
      ) : null;
    case "rename":
      return (
        <RenameModal
          open
          systemPath={systemPath}
          gamePaths={tool.paths}
          onClose={close}
          onDone={(moved) => {
            const one = tool.paths.length === 1 ? tool.paths[0] : undefined;
            void refresh(one ? moved[one] ?? one : undefined);
          }}
        />
      );
    case "review":
      return game ? (
        <GameReviewModal
          systemPath={systemPath}
          game={game}
          onClose={close}
          // Cada «Arreglar» abre la herramienta de ese juego (scrapear = la búsqueda manual)
          onFix={(fix) => openGameTool(fix, [game.path])}
        />
      ) : null;
    case "normalize":
      return game ? (
        <NormalizeMediaModal open systemPath={systemPath} gamePath={game.path} gameName={game.name} onClose={close} onDone={() => refresh(game.path)} />
      ) : null;
    case "clean":
      return game ? (
        <UnusedMediaModal open systemPath={systemPath} gamePath={game.path} gameName={game.name} onClose={close} onDone={() => refresh(game.path)} />
      ) : null;
    case "versions":
      return <VersionsModal open systemPath={systemPath} onlyPaths={tool.paths} onClose={close} onDone={reloadAll} />;
    case "move":
      return (
        <MoveGamesModal
          systemPath={systemPath}
          gamePaths={tool.paths}
          onClose={close}
          onDone={async () => {
            // Los juegos ya no están donde estaban
            useAppStore.getState().selectGame(null);
            close();
            await reloadAll();
          }}
        />
      );
  }
}
