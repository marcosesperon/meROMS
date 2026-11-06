import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../store/useAppStore";
import type { Game, SystemInfo, ViewMode } from "../types";

/*
 * Modo capturas (solo desarrollo): `MEROMS_SHOTS=1 pnpm tauri dev`.
 * La app recorre estas escenas con la biblioteca del perfil activo y, en cada una, se hace una
 * foto de su ventana (comando capture_window) que va a site/images/<nombre>.jpg para la web.
 * Para cambiar una captura: tocar su escena aquí (sistema, juego y qué se abre).
 */

let running = false;
// En desarrollo React lanza los efectos dos veces: el recorrido, una sola
let started = false;
/** ¿Se están sacando capturas? (mientras, no salen el aviso de donativo ni el de versión nueva) */
export const shotsRunning = () => running;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Espera a que algo sea verdad (o se rinde a los `ms`) */
async function waitFor(check: () => boolean, ms = 15000) {
  const end = Date.now() + ms;
  while (!check() && Date.now() < end) await sleep(100);
}

/** Pulsa el botón marcado con data-shot (lo espera si aún no está pintado) */
async function click(shot: string) {
  await waitFor(() => !!document.querySelector(`[data-shot="${shot}"]`), 5000);
  document.querySelector<HTMLElement>(`[data-shot="${shot}"]`)?.click();
  await sleep(400);
}

/** Espera a que terminen de cargar las imágenes que se ven, y un poco más (vídeos, PDF) */
async function settle(extra = 1200) {
  await waitFor(() => Array.from(document.images).every((img) => img.complete), 8000);
  await sleep(extra);
}

async function openSystem(name: string, view: ViewMode) {
  const s = useAppStore.getState();
  const system = s.systems.find((x: SystemInfo) => x.name === name);
  if (!system) throw new Error(`No system ${name}`);
  s.closeFullView();
  s.closeEditor();
  s.setSearchOpen(false);
  if (s.selectedSystem?.path !== system.path) {
    s.selectSystem(system);
    await waitFor(() => useAppStore.getState().games.length > 0 && !useAppStore.getState().loading);
  }
  s.setViewMode(view);
  useAppStore.getState().selectGame(null);
  await sleep(300);
}

function pickGame(match: (g: Game) => boolean) {
  const game = useAppStore.getState().games.find(match);
  if (!game) throw new Error("Game not found");
  useAppStore.getState().selectGame(game);
  document.querySelector(`[data-game-path="${CSS.escape(game.path)}"]`)?.scrollIntoView({ block: "center" });
}
const byPath = (path: string) => (g: Game) => g.path === path;

const shot = (name: string) => invoke("capture_window", { name });

/** Las escenas, en orden: nombre del fichero y cómo se llega a ella */
const SCENES: { name: string; run: () => Promise<void> }[] = [
  { name: "grid", run: async () => { await openSystem("mastersystem", "grid"); pickGame(byPath("./Alex Kidd in Miracle World.sms")); } },
  {
    name: "filters",
    run: async () => {
      await openSystem("mastersystem", "grid");
      await click("filters-toggle");
    },
  },
  {
    name: "status",
    run: async () => {
      await click("filters-toggle"); // cerrar la barra de filtros de la escena anterior
      pickGame(byPath("./Alex Kidd in Miracle World.sms"));
      await sleep(500);
      await click("status-menu");
    },
  },
  {
    // Buscador global (⌘K) con «al»: resultados de varios sistemas
    name: "search",
    run: async () => {
      await openSystem("cps1", "grid");
      useAppStore.getState().setSearchOpen(true);
      await waitFor(() => !!document.querySelector('[data-shot="global-search"]'), 3000);
      const input = document.querySelector<HTMLInputElement>('[data-shot="global-search"]');
      if (input) {
        // Escribir como lo haría el usuario (React escucha el evento «input»)
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, "al");
        input.dispatchEvent(new Event("input", { bubbles: true }));
      }
      await sleep(1500);
    },
  },
  { name: "table", run: async () => { await openSystem("mastersystem", "table"); pickGame(byPath("./Alex Kidd in Miracle World.sms")); } },
  { name: "system", run: async () => { await openSystem("cps1", "grid"); } },
  {
    name: "tools",
    run: async () => {
      await openSystem("cps1", "grid");
      pickGame(byPath("./sf2.zip"));
      await click("tools-menu");
    },
  },
  {
    name: "editor",
    run: async () => {
      document.body.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })); // cerrar el menú
      await openSystem("cps1", "grid");
      pickGame(byPath("./sf2.zip"));
      useAppStore.getState().openEditor();
    },
  },
  {
    name: "fullview",
    run: async () => {
      await openSystem("cps1", "grid");
      pickGame(byPath("./sf2.zip"));
      useAppStore.getState().openFullView();
      await click("tab-media");
    },
  },
  {
    name: "manual",
    run: async () => {
      await openSystem("mastersystem", "grid");
      pickGame(byPath("./Aladdin.sms"));
      useAppStore.getState().openFullView();
      await click("tab-manual");
      await sleep(2500); // el PDF tarda en pintarse
    },
  },
  {
    name: "metadata",
    run: async () => {
      if (useAppStore.getState().selectedGame?.path !== "./Aladdin.sms" || !useAppStore.getState().fullViewOpen) {
        await openSystem("mastersystem", "grid");
        pickGame(byPath("./Aladdin.sms"));
        useAppStore.getState().openFullView();
      }
      await click("tab-meta");
      await sleep(2500); // los hashes se calculan al abrir la pestaña
    },
  },
  {
    name: "settings-profiles",
    run: async () => {
      useAppStore.getState().closeFullView();
      useAppStore.getState().setCurrentView("settings");
      await click("settings-tab-profiles");
    },
  },
  {
    name: "settings-scraper",
    run: async () => {
      if (!document.querySelector('[data-shot="settings-tab-general"]')) {
        useAppStore.getState().closeFullView();
        useAppStore.getState().setCurrentView("settings");
      }
      await click("settings-tab-general");
      const policy = document.querySelector<HTMLElement>('[data-shot="scrape-policy"]');
      policy?.scrollIntoView({ block: "start" });
      // Un poco de margen para que se vea su título («Qué actualiza el scraper»)
      const box = policy?.closest<HTMLElement>(".overflow-y-auto");
      if (box) box.scrollTop -= 60;
    },
  },
];

/** Si la app se lanzó en modo capturas, recorre las escenas y la cierra al terminar */
export async function startShots() {
  if (!import.meta.env.DEV || started) return;
  started = true;
  const on = await invoke<boolean>("shots_mode").catch(() => false);
  if (!on) return;
  running = true;
  await invoke("shots_prepare");
  // Esperar a que estén los sistemas del perfil
  await waitFor(() => useAppStore.getState().systems.length > 0 && !useAppStore.getState().loading, 30000);
  const before = useAppStore.getState();
  const restore = { system: before.selectedSystem, view: before.viewMode };
  useAppStore.setState({ structurePromptOpen: false });
  // MEROMS_SHOTS_ONLY=table,metadata: solo esas (las escenas que dependen de la anterior,
  // como metadata tras manual, se lanzan igual: cada una se prepara sola si hace falta)
  const only = await invoke<string[]>("shots_only").catch((): string[] => []);
  for (const scene of SCENES.filter((s) => only.length === 0 || only.includes(s.name))) {
    try {
      await scene.run();
      await settle();
      console.info("[shots]", await shot(scene.name));
    } catch (e) {
      console.error("[shots]", scene.name, e);
    }
  }
  // Dejarlo como estaba (vista y sistema se guardan en los ajustes)
  if (document.querySelector('[data-shot="settings-close"]')) await click("settings-close");
  const s = useAppStore.getState();
  s.setViewMode(restore.view);
  if (restore.system) s.selectSystem(restore.system);
  await sleep(1500);
  running = false;
  await invoke("shots_done");
}
