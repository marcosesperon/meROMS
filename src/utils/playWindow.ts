import { getCurrentWindow } from "@tauri-apps/api/window";

/*
 * La ventana de meROMS mientras se juega: se oculta para que el emulador quede delante (en
 * pantalla completa, meROMS tiene su propio escritorio en macOS y el emulador se abría detrás) y
 * vuelve cuando el emulador se cierra. Si falla algo con la ventana, se sigue sin más: el juego
 * ya está lanzado.
 */

/** Lo que tarda macOS en la animación de salir de pantalla completa */
const FULLSCREEN_EXIT_WAIT = 800;
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Oculta meROMS. Si estaba a pantalla completa sale antes: ocultarla así deja un escritorio negro */
export async function hideWhilePlaying() {
  const win = getCurrentWindow();
  try {
    if (await win.isFullscreen()) {
      await win.setFullscreen(false);
      await wait(FULLSCREEN_EXIT_WAIT);
    }
    await win.hide();
  } catch {
    // Sin ocultar, el juego sigue igual
  }
}

/** Vuelve a enseñar meROMS al frente (a pantalla completa si sigue en el modo sofá) */
export async function showAfterPlaying(fullscreen: boolean) {
  const win = getCurrentWindow();
  try {
    await win.show();
    await win.setFocus();
    if (fullscreen) await win.setFullscreen(true);
  } catch {
    // Si no se puede, el usuario la abre desde el Dock o la barra de tareas
  }
}
