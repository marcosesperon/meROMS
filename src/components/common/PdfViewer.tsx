import { useEffect, useRef, useState } from "react";
import * as pdfjs from "pdfjs-dist";
import type { PDFDocumentLoadingTask, PDFDocumentProxy, RenderTask } from "pdfjs-dist";
// El «worker» de pdf.js va como fichero aparte (Vite lo copia y da su URL)
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { useTranslation } from "../../i18n";
import { ExpandIcon, CloseIcon } from "./Icons";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

interface PdfViewerProps {
  /** URL del PDF (protocolo media://) */
  url: string;
  /** «Abrir con…» el programa del sistema */
  onOpenExternal?: () => void;
}

// Zoom: «ajustar al ancho» (1) multiplicado por esto
const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

/**
 * Visor de PDF propio con pdf.js (manuales de los juegos): página a página, con zoom y una vista
 * ampliada que ocupa toda la ventana. Se pinta igual en macOS, Windows y Linux.
 */
export function PdfViewer({ url, onOpenExternal }: PdfViewerProps) {
  const { t } = useTranslation();
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [zoom, setZoom] = useState(2); // índice en ZOOMS (1 = ajustar al ancho)
  const [expanded, setExpanded] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const [boxWidth, setBoxWidth] = useState(0);

  // Cargar el documento (los bytes por el protocolo media://)
  useEffect(() => {
    let cancelled = false;
    let task: PDFDocumentLoadingTask | null = null;
    setDoc(null);
    setError(null);
    setPage(1);
    (async () => {
      try {
        const data = new Uint8Array(await (await fetch(url)).arrayBuffer());
        if (cancelled) return;
        task = pdfjs.getDocument({ data });
        const loaded = await task.promise;
        if (!cancelled) setDoc(loaded);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    // Al cambiar de manual o cerrar: se suelta el documento (y su worker)
    return () => {
      cancelled = true;
      task?.destroy();
    };
  }, [url]);

  // El ancho del hueco manda en el zoom «ajustar al ancho»
  useEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const observer = new ResizeObserver(() => setBoxWidth(box.clientWidth));
    observer.observe(box);
    return () => observer.disconnect();
  }, [expanded]);

  // Pintar la página
  useEffect(() => {
    if (!doc || !canvasRef.current || boxWidth === 0) return;
    let task: RenderTask | null = null;
    let cancelled = false;
    (async () => {
      const p = await doc.getPage(page);
      if (cancelled) return;
      const base = p.getViewport({ scale: 1 });
      const scale = ((boxWidth - 16) / base.width) * ZOOMS[zoom];
      // A la densidad de la pantalla, para que el texto no salga borroso
      const ratio = window.devicePixelRatio || 1;
      const viewport = p.getViewport({ scale: scale * ratio });
      const canvas = canvasRef.current!;
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      canvas.style.width = `${viewport.width / ratio}px`;
      canvas.style.height = `${viewport.height / ratio}px`;
      task = p.render({ canvas, viewport });
      await task.promise.catch(() => {});
    })();
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [doc, page, zoom, boxWidth]);

  const pages = doc?.numPages ?? 0;
  const go = (n: number) => setPage((p) => Math.min(Math.max(1, p + n), pages || 1));

  // En la vista ampliada, las flechas pasan página y Esc la cierra
  useEffect(() => {
    if (!expanded) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setExpanded(false);
      else if (e.key === "ArrowLeft") go(-1);
      else if (e.key === "ArrowRight") go(1);
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [expanded, pages]);

  const btn = "px-2.5 py-1 text-sm rounded bg-zinc-800 text-zinc-300 hover:bg-zinc-700 disabled:opacity-40";
  const toolbar = (
    <div className="flex items-center gap-2 py-2 shrink-0">
      <button onClick={() => go(-1)} disabled={page <= 1} className={btn} title={t("pdf.prev")}>‹</button>
      <span className="text-xs text-zinc-400 tabular-nums">{pages ? t("pdf.page", { n: page, total: pages }) : "…"}</span>
      <button onClick={() => go(1)} disabled={page >= pages} className={btn} title={t("pdf.next")}>›</button>
      <span className="w-3" />
      <button onClick={() => setZoom((z) => Math.max(0, z - 1))} disabled={zoom === 0} className={btn} title={t("pdf.zoomOut")}>−</button>
      <span className="text-xs text-zinc-400 w-10 text-center tabular-nums">{Math.round(ZOOMS[zoom] * 100)}%</span>
      <button onClick={() => setZoom((z) => Math.min(ZOOMS.length - 1, z + 1))} disabled={zoom === ZOOMS.length - 1} className={btn} title={t("pdf.zoomIn")}>+</button>
      <span className="flex-1" />
      {onOpenExternal && (
        <button onClick={onOpenExternal} className={btn}>{t("pdf.openExternal")}</button>
      )}
      <button onClick={() => setExpanded((v) => !v)} className={btn} title={t(expanded ? "pdf.collapse" : "pdf.expand")}>
        {expanded ? <CloseIcon size={14} /> : <ExpandIcon size={14} />}
      </button>
    </div>
  );

  const body = (
    <div ref={boxRef} className="flex-1 min-h-0 overflow-auto rounded-lg bg-black/60">
      {error ? (
        <p className="p-6 text-sm text-red-300">{t("pdf.error", { error })}</p>
      ) : !doc ? (
        <p className="p-6 text-sm text-zinc-500">{t("pdf.loading")}</p>
      ) : (
        // Centrada mientras quepa; si se amplía más, se desplaza
        <div className="min-w-full w-max p-2 flex justify-center">
          <canvas ref={canvasRef} className="shadow-lg shadow-black/60" />
        </div>
      )}
    </div>
  );

  // Ampliado: una capa sobre toda la ventana (sin la pantalla completa del sistema, que en el
  // WebView no siempre va igual)
  if (expanded) {
    return (
      <div className="fixed inset-0 z-[70] bg-zinc-950/95 flex flex-col px-6 pb-6">
        {toolbar}
        {body}
      </div>
    );
  }
  return (
    <div className="h-full flex flex-col min-h-0">
      {toolbar}
      {body}
    </div>
  );
}
