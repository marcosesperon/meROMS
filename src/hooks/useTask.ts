import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { cancelTask } from "../commands";

/** Por dónde va un proceso largo (evento `task-progress` de `progress.rs`) */
export interface TaskProgressInfo {
  /** Clave de la fase («reading», «hashing»…), se traduce con `task.phase.<clave>` */
  phase: string;
  current: number;
  /** 0 = aún no se sabe cuántos hay */
  total: number;
  /** Fichero o juego en curso */
  item: string;
}

interface TaskEvent extends TaskProgressInfo {
  task: string;
}

// Un solo listener para toda la app: cada tarea se apunta con su id y recibe lo suyo.
// Así no se pierden los primeros avisos mientras se monta un listener por tarea.
const subscribers = new Map<string, (p: TaskProgressInfo) => void>();
let listening = false;
function ensureListener() {
  if (listening) return;
  listening = true;
  listen<TaskEvent>("task-progress", (e) => {
    const { task, ...info } = e.payload;
    subscribers.get(task)?.(info);
  }).catch(() => {
    listening = false;
  });
}

let counter = 0;
export const newTaskId = () => `t${Date.now().toString(36)}-${++counter}`;

/**
 * Para usar fuera de un componente (el store): recibe el progreso de la tarea `task` en `cb`.
 * Devuelve la función que deja de escucharla.
 */
export function subscribeTask(task: string, cb: (p: TaskProgressInfo) => void): () => void {
  ensureListener();
  subscribers.set(task, cb);
  return () => {
    subscribers.delete(task);
  };
}

/** ¿El error es el de un proceso cancelado por el usuario? */
export const isCancelled = (e: unknown) => String(e) === "cancelled";

/**
 * Un proceso largo con progreso y Cancelar. `run(fn)` da a `fn` un id de tarea que se pasa al
 * comando; mientras dura, `progress` trae la fase, «N de M» y el fichero en curso. Si se cancela
 * un análisis, `run` devuelve `null` (no hay nada que enseñar); los errores de verdad se lanzan.
 */
export function useTask() {
  const [progress, setProgress] = useState<TaskProgressInfo | null>(null);
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const current = useRef<string | null>(null);
  const mounted = useRef(true);
  // Se ha pulsado Cancelar en la tarea en curso (para saber, al acabar un cambio, si se paró)
  const cancelRequested = useRef(false);

  useEffect(() => {
    ensureListener();
    mounted.current = true;
    return () => {
      mounted.current = false;
      // Si se cierra la ventana a medias, el proceso no sigue trabajando para nadie
      if (current.current) {
        subscribers.delete(current.current);
        cancelTask(current.current).catch(() => {});
      }
    };
  }, []);

  const run = useCallback(async <T,>(fn: (task: string) => Promise<T>): Promise<T | null> => {
    const task = newTaskId();
    current.current = task;
    subscribers.set(task, (p) => mounted.current && current.current === task && setProgress(p));
    cancelRequested.current = false;
    setProgress(null);
    setCancelling(false);
    setRunning(true);
    try {
      return await fn(task);
    } catch (e) {
      if (isCancelled(e)) return null;
      throw e;
    } finally {
      subscribers.delete(task);
      if (current.current === task) {
        current.current = null;
        if (mounted.current) {
          setRunning(false);
          setProgress(null);
          setCancelling(false);
        }
      }
    }
  }, []);

  const cancel = useCallback(() => {
    if (!current.current) return;
    cancelRequested.current = true;
    setCancelling(true);
    cancelTask(current.current).catch(() => {});
  }, []);

  /** ¿Se pulsó Cancelar en la última tarea? (un cambio parado a medias devuelve lo hecho) */
  const wasCancelled = useCallback(() => cancelRequested.current, []);

  return { run, cancel, wasCancelled, running, cancelling, progress };
}
