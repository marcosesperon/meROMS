import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Todo lo de las versiones nuevas: buscar, avisar e instalar. Va aparte del store principal
// para no mezclarlo con la biblioteca. Lo usan la banda de aviso, su ventana y «Acerca de».

export type UpdatePhase =
  | "idle" // sin mirar todavía (o una comprobación automática que falló sin avisar)
  | "checking"
  | "upToDate"
  | "available"
  | "downloading"
  | "installing" // descargada: se está instalando y la app se reiniciará
  | "error";

interface UpdateState {
  phase: UpdatePhase;
  /** La versión nueva encontrada (con sus notas), mientras haya una */
  update: Update | null;
  /** Avance de la descarga, 0..1 (null si no se sabe el tamaño) */
  progress: number | null;
  error: string | null;
  /** La × de la banda: la oculta hasta el siguiente arranque */
  bannerDismissed: boolean;
  modalOpen: boolean;
  /** Busca una versión nueva. `manual` = la pidió el usuario: entonces sí se enseñan los errores */
  checkNow: (manual?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismissBanner: () => void;
  openModal: () => void;
  closeModal: () => void;
}

const isBusy = (phase: UpdatePhase) => phase === "checking" || phase === "downloading" || phase === "installing";

export const useUpdateStore = create<UpdateState>((set, get) => ({
  phase: "idle",
  update: null,
  progress: null,
  error: null,
  bannerDismissed: false,
  modalOpen: false,

  checkNow: async (manual = false) => {
    if (isBusy(get().phase)) return;
    // Si ya hay una encontrada, no se vuelve a buscar en segundo plano (perdería la que hay)
    if (!manual && get().update) return;
    set({ phase: "checking", error: null });
    try {
      const update = await check();
      set(update ? { phase: "available", update } : { phase: "upToDate", update: null });
    } catch (e) {
      // Sin conexión, en desarrollo o sin release publicada: en automático no molesta
      console.warn("Update check failed:", e);
      set(manual ? { phase: "error", error: String(e) } : { phase: get().update ? "available" : "idle" });
    }
  },

  install: async () => {
    const { update } = get();
    if (!update || isBusy(get().phase)) return;
    set({ phase: "downloading", progress: 0, error: null });
    let total = 0;
    let received = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          set({ progress: total ? 0 : null });
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          if (total) set({ progress: Math.min(1, received / total) });
        } else if (event.event === "Finished") {
          set({ phase: "installing", progress: 1 });
        }
      });
      // En Windows el instalador ya cierra la app; en macOS y Linux hay que reiniciarla
      await relaunch();
    } catch (e) {
      console.error("Update install failed:", e);
      set({ phase: "error", error: String(e) });
    }
  },

  dismissBanner: () => set({ bannerDismissed: true }),
  openModal: () => set({ modalOpen: true }),
  closeModal: () => {
    if (isBusy(get().phase)) return; // a mitad de descarga no se cierra
    set({ modalOpen: false });
  },
}));
