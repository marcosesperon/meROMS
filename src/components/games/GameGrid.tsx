import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
import { useVirtualRows } from "../../hooks/useVirtualRows";
import { registerGameScroller } from "../../utils/scrollToGame";
import type { Game } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { GameCard } from "./GameCard";
import { cardAspectRatio, cardWidthFor } from "../../utils/cartridgeSizes";
import { boxes3dOn, getSystemView, shapeField } from "../../utils/systemView";
import { nameText, sortGames, sortnameText } from "../../utils/playStats";
import { LetterBar } from "./LetterBar";
import { systemBackdrop } from "../../utils/systemInfo";

interface GameGridProps {
  games: Game[];
}

/** Separación entre tarjetas (px) y relleno del contenedor; la cuenta de filas los necesita */
const GAP_X = 12;
const GAP_Y = 16;
const PADDING = 20;

export function GameGrid({ games }: GameGridProps) {
  const selectedSystem = useAppStore((s) => s.selectedSystem);
  const settings = useAppStore((s) => s.settings);
  // Forma de las tarjetas: la elegida para el sistema en el perfil, la que más se repite en el
  // medio que se enseña (medida por el store al cargar el sistema) o la tabla fija
  const view = getSystemView(settings, selectedSystem?.name);
  // Medio cuya forma se mide (con cajas 3D, la carátula 2D, que es lo que va al frente)
  const gridField = shapeField(settings, selectedSystem?.name, "grid");
  const mediaShape = useAppStore((s) => s.mediaShape);
  const refreshMediaShape = useAppStore((s) => s.refreshMediaShape);
  const measured = mediaShape && mediaShape.system === selectedSystem?.path && mediaShape.field === gridField ? mediaShape : null;
  const aspectRatio = cardAspectRatio(selectedSystem?.name || "", view.aspect_ratio, measured);
  const cardWidth = cardWidthFor(aspectRatio);
  // Cajas 3D: lo elegido en el sistema o, si no, lo de Ajustes > General
  const box3d = boxes3dOn(settings, selectedSystem?.name);
  // Al cambiar el medio de la cuadrícula (en Ajustes o en el sistema), se mide el nuevo
  useEffect(() => {
    if (selectedSystem && (mediaShape?.system !== selectedSystem.path || mediaShape?.field !== gridField)) {
      refreshMediaShape(selectedSystem.path);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSystem?.path, gridField]);
  // Orden elegido junto al cambio de vista (por defecto, por nombre o «nombre para ordenar»
  // como ES-DE; los que no tienen el dato del orden van al final)
  const gridSort = useAppStore((s) => s.settings.grid_sort);
  const sorted = useMemo(() => sortGames(games, gridSort), [games, gridSort]);
  // Clave de cada tarjeta: su ruta. Hay gamelists con la misma ruta en varias entradas (p. ej.
  // una por región) y, con claves repetidas, React se lía al cambiar la lista y deja tarjetas
  // viejas a la vista; a la segunda y siguientes se les añade «#2», «#3»…
  const keys = useMemo(() => {
    const seen = new Map<string, number>();
    return sorted.map((g) => {
      const n = (seen.get(g.path) ?? 0) + 1;
      seen.set(g.path, n);
      return n === 1 ? g.path : `${g.path}#${n}`;
    });
  }, [sorted]);
  // El orden que se ve, para el anterior/siguiente de la ficha a pantalla completa
  const setVisibleOrder = useAppStore((s) => s.setVisibleOrder);
  useEffect(() => setVisibleOrder(sorted.map((g) => g.path)), [sorted, setVisibleOrder]);
  // Barra de letras: con los dos órdenes por nombre, sacando la letra del texto por el que se ordena
  const byName = !gridSort || gridSort === "name" || gridSort === "sortname";
  const letterText = gridSort === "sortname" ? sortnameText : nameText;

  // Solo se pintan las filas que se ven (con miles de juegos, montar todas las tarjetas hace la
  // lista lenta y pide todas las imágenes a la vez). Columnas: las que caben con el ancho mínimo
  // de tarjeta, como hacía «auto-fill»; alto de fila: el de una tarjeta pintada.
  const scrollRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const [innerWidth, setInnerWidth] = useState(0);
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => setInnerWidth(el.clientWidth - PADDING * 2);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const columns = Math.max(1, Math.floor((innerWidth + GAP_X) / (cardWidth + GAP_X)));
  // Mientras no hay una tarjeta medida, una estimación con su forma (carátula + dos líneas de texto)
  const [aw, ah] = aspectRatio.split("/").map(Number);
  const estimate = ((innerWidth - GAP_X * (columns - 1)) / columns) * (aw > 0 && ah > 0 ? ah / aw : 4 / 3) + 52;
  const [cardHeight, setCardHeight] = useState(0);
  useLayoutEffect(() => {
    // Con decimales: offsetHeight redondea y, con cientos de filas, el error se acumula.
    // Por debajo de 1 px no se toca: si dos filas midieran casi lo mismo, cambiar la medida
    // cambia qué fila se pinta primero, que vuelve a cambiar la medida… y no para nunca.
    const height = (gridRef.current?.firstElementChild as HTMLElement | null)?.getBoundingClientRect().height ?? 0;
    if (height > 0 && Math.abs(height - cardHeight) >= 1) setCardHeight(height);
  });
  const rowCount = Math.ceil(sorted.length / columns);
  const stride = (cardHeight || estimate) + GAP_Y;
  const rows = useVirtualRows({ containerRef: scrollRef, rowCount, rowStride: stride, offset: PADDING, inset: PADDING });
  const shown = sorted.slice(rows.first * columns, rows.last * columns);

  // Ir a un juego (teclado, barra de letras, búsqueda, ficha completa): a su fila
  const latest = useRef({ sorted, columns, scrollToRow: rows.scrollToRow });
  latest.current = { sorted, columns, scrollToRow: rows.scrollToRow };
  useEffect(
    () =>
      registerGameScroller((path, block) => {
        const { sorted, columns, scrollToRow } = latest.current;
        const index = sorted.findIndex((g) => g.path === path);
        if (index < 0) return false;
        scrollToRow(Math.floor(index / columns), block);
        return true;
      }),
    []
  );

  useListKeyboard({
    items: sorted,
    containerRef: scrollRef,
    getColumns: () => columns,
    getRowsPerPage: () => rows.rowsPerPage,
    scrollToIndex: (index) => rows.scrollToRow(Math.floor(index / columns), "nearest"),
  });

  return (
    // Fondo con los colores del sistema (como el carrusel); fijo, las tarjetas se desplazan encima
    <div className="flex-1 flex overflow-hidden" style={{ background: systemBackdrop(selectedSystem?.name || "") }}>
      <div ref={scrollRef} className="overflow-auto flex-1" style={{ padding: PADDING }}>
        {/* Un bloque con el alto de todas las filas y, dentro, las pintadas movidas con transform
            hasta su sitio. Antes eran huecos arriba y abajo que cambiaban de alto al desplazarse, y
            WebKit (macOS) a veces no redibujaba las filas nuevas hasta el siguiente scroll: se veía
            el fondo vacío un rato */}
        <div className="relative" style={{ height: Math.max(0, rowCount * stride - GAP_Y) }}>
          <div
            ref={gridRef}
            className="grid absolute inset-x-0 top-0"
            style={{
              gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
              columnGap: GAP_X,
              rowGap: GAP_Y,
              transform: `translateY(${rows.before}px)`,
            }}
          >
            {shown.map((game, i) => (
              <GameCard key={keys[rows.first * columns + i]} game={game} aspectRatio={aspectRatio} box3d={box3d} />
            ))}
          </div>
        </div>
      </div>
      {/* Saltar a letra: solo con la lista por nombre (con otro orden las letras van salteadas) */}
      {byName && (
        <LetterBar
          games={sorted}
          nameOf={letterText}
          visibleFrom={rows.visibleFirst * columns}
          visibleTo={rows.visibleLast * columns}
          onJump={(path) => {
            const index = sorted.findIndex((g) => g.path === path);
            if (index >= 0) rows.scrollToRow(Math.floor(index / columns), "start");
          }}
        />
      )}
    </div>
  );
}

