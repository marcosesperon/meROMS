import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
  type ColumnResizeMode,
  type ColumnSizingState,
} from "@tanstack/react-table";
import { useEffect, useMemo, useRef, useState } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
import type { Game } from "../../types";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { StarIcon, StarOutlineIcon, EyeSlashIcon, ChildIcon } from "../common/Icons";
import { getFlag } from "../../utils/flags";
import { resolveImageSrc } from "../../utils/images";
import { DatTag } from "../common/DatTag";
import { ColumnsMenu } from "./ColumnsMenu";
import { formatPlayTime } from "../../utils/playStats";
import { clickGame, doubleClickGame } from "../../utils/gameClick";
import { LetterBar } from "./LetterBar";
import { statusBadge, statusText } from "../../utils/gameStatus";

// Texto de la columna Nombre (de él sale la letra de la barra)
const nameText = (g: Game) => g.name || g.path;

const columnHelper = createColumnHelper<Game>();

interface GameTableProps {
  games: Game[];
}

function ToggleCell({
  value,
  gamePath,
  field,
  ActiveIcon,
  InactiveIcon,
  activeClass,
  inactiveClass,
}: {
  value?: string;
  gamePath: string;
  field: "favorite" | "hidden" | "kidgame";
  ActiveIcon: React.FC<{ size?: number; className?: string }>;
  InactiveIcon?: React.FC<{ size?: number; className?: string }>;
  activeClass: string;
  inactiveClass: string;
}) {
  const toggleGameFlag = useAppStore((s) => s.toggleGameFlag);
  const isActive = value === "true";
  const Icon = isActive ? ActiveIcon : (InactiveIcon || ActiveIcon);

  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        toggleGameFlag(gamePath, field);
      }}
      className={`p-1 rounded transition-colors ${isActive ? activeClass : inactiveClass}`}
      title={field}
    >
      <Icon size={14} />
    </button>
  );
}

/** Miniatura de una fila: el medio de la cuadrícula (o carátula / captura); nada si no hay o falla */
function RowThumb({ game }: { game: Game }) {
  const gridMediaField = useAppStore((s) => s.settings.grid_media_field) || "image";
  const systemPath = useAppStore((s) => s.selectedSystem?.path) || "";
  const [failed, setFailed] = useState(false);
  const value = (game as unknown as Record<string, string | undefined>)[gridMediaField] || game.image || game.thumbnail;
  const src = resolveImageSrc(value, systemPath);
  return (
    <div className="w-9 h-9 rounded bg-zinc-800 overflow-hidden flex items-center justify-center">
      {src && !failed && (
        <img src={src} alt="" loading="lazy" onError={() => setFailed(true)} className="max-w-full max-h-full object-contain" />
      )}
    </div>
  );
}

// Siempre visibles y al principio (congeladas); el resto se elige en el menú de columnas
const FIXED_COLUMNS = ["thumb", "name"];
// Las de siempre, si no se ha guardado otra elección
const DEFAULT_COLUMNS = ["genre", "developer", "publisher", "players", "rating", "releasedate", "flags"];


// Columnas «congeladas» en el scroll horizontal: la miniatura y el nombre
const FROZEN = new Set(["thumb", "name"]);
// Fondo opaco de las celdas congeladas (si fuera transparente, se verían las columnas que
// pasan por debajo): el mismo color que la fila, ya mezclado con el fondo
const FROZEN_BG = {
  normal: "bg-zinc-900 group-hover:bg-[color-mix(in_srgb,var(--color-zinc-800)_50%,var(--color-zinc-900))]",
  selected: "bg-[color-mix(in_srgb,var(--color-indigo-600)_15%,var(--color-zinc-900))]",
};

// Lo que necesita el menú de columnas (se rellena en cada render de la tabla; la cabecera de la
// miniatura se define una sola vez y lo lee de aquí)
const columnsMenuProps: { current: Parameters<typeof ColumnsMenu>[0] | null } = { current: null };
function ColumnsMenuSlot() {
  return columnsMenuProps.current ? <ColumnsMenu {...columnsMenuProps.current} /> : null;
}

// Identificador de una columna: las de texto (accessor con nombre de campo) no traen `id`,
// lo lleva `accessorKey`; si se mira solo `id` todas salen vacías y se pisan entre sí
function columnId(c: { id?: string }): string {
  return c.id ?? (c as { accessorKey?: string }).accessorKey ?? "";
}

export function GameTable({ games }: GameTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnResizeMode] = useState<ColumnResizeMode>("onChange");
  const missingRomPaths = useAppStore((s) => s.missingRomPaths);
  const multiSelect = useAppStore((s) => s.multiSelect);
  const selectedPaths = useAppStore((s) => s.selectedPaths);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const statuses = useAppStore((s) => s.statuses);
  const { t } = useTranslation();

  // Todas las columnas posibles; las que se ven (y en qué orden) salen de Ajustes
  const allColumns = useMemo(
    () => [
      // Miniatura: el mismo medio que la cuadrícula (Ajustes → General), en pequeño
      columnHelper.display({
        id: "thumb",
        // El botón del menú de columnas va en la cabecera de la miniatura
        header: () => <ColumnsMenuSlot />,
        cell: (info) => <RowThumb game={info.row.original} />,
        size: 56,
        minSize: 56,
        enableResizing: false,
        enableSorting: false,
      }),
      columnHelper.accessor("name", {
        header: t("table.name"),
        cell: (info) => (
          <span className="flex items-center gap-2 min-w-0">
            <span className="truncate">{info.getValue() || info.row.original.path}</span>
            <DatTag path={info.row.original.path} className="shrink-0" />
          </span>
        ),
        size: 250,
        minSize: 100,
      }),
      columnHelper.accessor("genre", {
        header: t("table.genre"),
        cell: (info) => info.getValue() || "",
        size: 120,
        minSize: 60,
      }),
      columnHelper.accessor("developer", {
        header: t("table.developer"),
        cell: (info) => info.getValue() || "",
        size: 150,
        minSize: 80,
      }),
      columnHelper.accessor("publisher", {
        header: t("table.publisher"),
        cell: (info) => info.getValue() || "",
        size: 150,
        minSize: 80,
      }),
      columnHelper.accessor("players", {
        header: t("table.players"),
        cell: (info) => info.getValue() || "",
        size: 80,
        minSize: 50,
      }),
      columnHelper.accessor("rating", {
        header: t("table.rating"),
        cell: (info) => formatRating(info.getValue()),
        size: 90,
        minSize: 60,
      }),
      columnHelper.accessor("releasedate", {
        header: t("table.release"),
        cell: (info) => formatReleaseDate(info.getValue()),
        size: 120,
        minSize: 80,
      }),
      columnHelper.display({
        id: "flags",
        header: () => <span className="text-center block">★</span>,
        // Favorito, oculto, para niños y la bandera: con los anchos fijos de la tabla ya no
        // se ensancha sola, así que el ancho tiene que dar para los cuatro
        size: 140,
        minSize: 140,
        enableResizing: false,
        cell: (info) => {
          const game = info.row.original;
          const flag = getFlag(game.lang);
          return (
            <div className="flex items-center gap-0.5">
              <ToggleCell
                value={game.favorite}
                gamePath={game.path}
                field="favorite"
                ActiveIcon={StarIcon}
                InactiveIcon={StarOutlineIcon}
                activeClass="text-yellow-400 hover:text-yellow-300"
                inactiveClass="text-zinc-600 hover:text-zinc-400"
              />
              <ToggleCell
                value={game.hidden}
                gamePath={game.path}
                field="hidden"
                ActiveIcon={EyeSlashIcon}
                activeClass="text-red-400 hover:text-red-300"
                inactiveClass="text-zinc-600 hover:text-zinc-400"
              />
              <ToggleCell
                value={game.kidgame}
                gamePath={game.path}
                field="kidgame"
                ActiveIcon={ChildIcon}
                activeClass="text-green-400 hover:text-green-300"
                inactiveClass="text-zinc-600 hover:text-zinc-400"
              />
              {flag && <span className="text-sm ml-1" title={game.lang}>{flag}</span>}
            </div>
          );
        },
      }),
      // Columnas extra (ocultas por defecto; se activan en el menú de columnas)
      columnHelper.accessor("region", { header: t("table.region"), cell: (i) => i.getValue() || "", size: 90, minSize: 60 }),
      columnHelper.accessor("lang", {
        header: t("table.lang"),
        cell: (i) => [getFlag(i.getValue()), i.getValue()].filter(Boolean).join(" "),
        size: 90,
        minSize: 60,
      }),
      columnHelper.accessor("family", { header: t("table.family"), cell: (i) => i.getValue() || "", size: 150, minSize: 80 }),
      columnHelper.accessor("playcount", {
        header: t("table.playcount"),
        cell: (i) => i.getValue() || "",
        size: 90,
        minSize: 60,
        sortingFn: (a, b) => Number(a.original.playcount ?? 0) - Number(b.original.playcount ?? 0),
      }),
      columnHelper.accessor("lastplayed", { header: t("table.lastplayed"), cell: (i) => formatReleaseDate(i.getValue()), size: 120, minSize: 80 }),
      columnHelper.accessor("gametime", {
        header: t("table.gametime"),
        cell: (i) => formatPlayTime(i.getValue()),
        size: 100,
        minSize: 70,
        sortingFn: (a, b) => Number(a.original.gametime ?? 0) - Number(b.original.gametime ?? 0),
      }),
      // Estado de juego (meROMS): icono y texto; se ordena por el texto
      columnHelper.accessor((g) => statusText(statuses.get(g.path), t), {
        id: "status",
        header: t("status.column"),
        cell: (i) => {
          const badge = statusBadge(statuses.get(i.row.original.path));
          if (!badge) return "";
          return (
            <span className="flex items-center gap-1.5 min-w-0" title={i.getValue()}>
              <badge.icon size={13} className={`shrink-0 ${badge.color}`} />
              <span className="truncate">{i.getValue()}</span>
            </span>
          );
        },
        size: 150,
        minSize: 80,
      }),
      columnHelper.accessor("arcadesystemname", { header: t("table.arcadesystem"), cell: (i) => i.getValue() || "", size: 110, minSize: 70 }),
      columnHelper.accessor("sortname", { header: t("form.sortname"), cell: (i) => i.getValue() || "", size: 180, minSize: 80 }),
      columnHelper.accessor("path", {
        header: t("table.path"),
        cell: (i) => <span className="font-mono text-xs text-zinc-400">{i.getValue()}</span>,
        size: 220,
        minSize: 100,
      }),
      columnHelper.accessor((g) => (g.video ? 1 : 0), {
        id: "hasvideo",
        header: t("table.hasVideo"),
        cell: (i) => (i.getValue() ? "✓" : ""),
        size: 70,
        minSize: 50,
      }),
      columnHelper.accessor((g) => (g.manual ? 1 : 0), {
        id: "hasmanual",
        header: t("table.hasManual"),
        cell: (i) => (i.getValue() ? "✓" : ""),
        size: 70,
        minSize: 50,
      }),
    ],
    [t, statuses]
  );

  // Columnas visibles: miniatura y nombre siempre (congeladas), y el resto en el orden guardado
  const savedColumns = useAppStore((s) => s.settings.table_columns);
  const patchSettings = useAppStore((s) => s.patchSettings);
  const byId = useMemo(() => new Map(allColumns.map((c) => [columnId(c), c])), [allColumns]);
  const visibleIds = useMemo(() => {
    // Lo guardado puede traer identificadores vacíos o que ya no existen: se descartan
    const saved = savedColumns?.map((c) => c.id).filter((id) => id && byId.has(id) && !FIXED_COLUMNS.includes(id));
    return saved && saved.length > 0 ? saved : DEFAULT_COLUMNS;
  }, [savedColumns, byId]);
  const columns = useMemo(
    () => [...FIXED_COLUMNS, ...visibleIds].map((id) => byId.get(id)!).filter(Boolean),
    [visibleIds, byId]
  );

  // Anchos: los guardados al empezar; al cambiarlos se guardan (con un pequeño retraso)
  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>(() =>
    Object.fromEntries((savedColumns ?? []).filter((c) => c.width).map((c) => [c.id, c.width!]))
  );
  function saveColumns(ids: string[], sizing: ColumnSizingState) {
    patchSettings({
      table_columns: ["name", ...ids].map((id) => ({ id, width: sizing[id] ? Math.round(sizing[id]) : undefined })),
    });
  }
  // Menú de columnas: todas menos las fijas, con su título
  const optionalColumns = useMemo(
    () =>
      allColumns
        .filter((c) => !FIXED_COLUMNS.includes(columnId(c)))
        .map((c) => ({ id: columnId(c), label: typeof c.header === "string" ? c.header : t("table.flags") })),
    [allColumns, t]
  );
  columnsMenuProps.current = {
    all: optionalColumns,
    visible: visibleIds,
    onChange: (ids) => saveColumns(ids, columnSizing),
    onReset: () => {
      setColumnSizing({});
      patchSettings({ table_columns: undefined });
    },
  };

  useEffect(() => {
    if (Object.keys(columnSizing).length > 0) saveColumns(visibleIds, columnSizing);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [columnSizing]);

  const table = useReactTable({
    data: games,
    columns,
    state: { sorting, columnSizing },
    onSortingChange: setSorting,
    onColumnSizingChange: setColumnSizing,
    columnResizeMode,
    enableColumnResizing: true,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  // Teclado: las filas en el orden en que se ven (con la columna de orden elegida)
  const scrollRef = useRef<HTMLDivElement>(null);
  const rows = table.getRowModel().rows;
  const ordered = useMemo(() => rows.map((r) => r.original), [rows]);
  useListKeyboard({
    items: ordered,
    containerRef: scrollRef,
    getColumns: () => 1,
  });
  // El orden que se ve, para el anterior/siguiente de la ficha a pantalla completa
  const setVisibleOrder = useAppStore((s) => s.setVisibleOrder);
  useEffect(() => setVisibleOrder(ordered.map((g) => g.path)), [ordered, setVisibleOrder]);
  // Saltar a letra: solo con la tabla ordenada por la columna Nombre (sin orden, va como la gamelist)
  const nameSort = sorting[0]?.id === "name" ? sorting[0] : undefined;

  return (
    <div className="flex-1 flex overflow-hidden">
      <div ref={scrollRef} className="overflow-auto flex-1">
        <table
          className="text-sm border-collapse table-fixed"
          // Como mínimo todo el ancho: lo que sobra se lo queda una columna de relleno al final
          // (estirar las de verdad descuadraría las congeladas, que se colocan con sus anchos)
          style={{ width: table.getCenterTotalSize(), minWidth: "100%" }}
        >
          <thead className="bg-zinc-800">
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    // Cada cabecera se queda arriba al bajar (top-0); las congeladas además a la
                    // izquierda y por encima de todo (esquina): si no, las celdas congeladas del
                    // cuerpo las tapaban al hacer scroll vertical
                    className={`sticky top-0 bg-zinc-800 text-left px-4 py-2.5 text-zinc-400 font-medium border-b border-zinc-700 select-none ${
                      FROZEN.has(header.column.id) ? "z-30" : "z-20"
                    } ${header.column.id === "name" ? "shadow-[inset_-1px_0_0_var(--color-zinc-700)]" : ""}`}
                    style={{
                      width: header.getSize(),
                      left: FROZEN.has(header.column.id) ? header.column.getStart() : undefined,
                    }}
                  >
                    <div
                      className="flex items-center gap-1 cursor-pointer hover:text-zinc-200 transition-colors"
                      onClick={header.column.getToggleSortingHandler()}
                    >
                      {flexRender(
                        header.column.columnDef.header,
                        header.getContext()
                      )}
                      {{
                        asc: " ↑",
                        desc: " ↓",
                      }[header.column.getIsSorted() as string] ?? ""}
                    </div>
                    {/* Resize handle — wide hit area, thin visible line */}
                    {header.column.getCanResize() && (
                      <div
                        onMouseDown={header.getResizeHandler()}
                        onTouchStart={header.getResizeHandler()}
                        onDoubleClick={() => header.column.resetSize()}
                        className="absolute right-0 top-0 h-full w-4 cursor-col-resize select-none touch-none group/resize -translate-x-1/2"
                      >
                        <div
                          className={`absolute right-[7px] top-0 h-full w-0.5 transition-colors ${
                            header.column.getIsResizing()
                              ? "bg-indigo-500"
                              : "bg-transparent group-hover/resize:bg-zinc-500"
                          }`}
                        />
                      </div>
                    )}
                  </th>
                ))}
                <th className="sticky top-0 z-20 bg-zinc-800 border-b border-zinc-700" />
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
                data-game-path={row.original.path}
                onClick={() => clickGame(row.original)}
                onDoubleClick={() => doubleClickGame(row.original)}
                // scroll-mt: que al subir con el teclado la fila no quede bajo la cabecera fija
                className={`group cursor-pointer scroll-mt-11 border-b border-zinc-800 transition-colors ${
                  (multiSelect ? selectedPaths.has(row.original.path) : selectedGame?.path === row.original.path)
                    ? "bg-indigo-600/15"
                    : "hover:bg-zinc-800/50"
                } ${missingRomPaths.has(row.original.path) ? "line-through decoration-red-500/60" : ""}`}
                title={missingRomPaths.has(row.original.path) ? t("missingRoms.badge") : undefined}
              >
                {row.getVisibleCells().map((cell) => {
                  const frozen = FROZEN.has(cell.column.id);
                  const rowSelected = multiSelect ? selectedPaths.has(row.original.path) : selectedGame?.path === row.original.path;
                  // Juego sin ROM: atenuado. En las celdas congeladas solo el contenido, para que
                  // su fondo siga siendo opaco y no se vea lo que pasa por debajo
                  const dim = missingRomPaths.has(row.original.path) ? (frozen ? "[&>*]:opacity-50" : "opacity-50") : "";
                  return (
                  <td
                    key={cell.id}
                    className={`${cell.column.id === "thumb" ? "pl-4 pr-0 py-1" : "px-4 py-2.5"} text-zinc-300 truncate ${
                      frozen ? `sticky z-10 ${rowSelected ? FROZEN_BG.selected : FROZEN_BG.normal}` : ""
                    } ${cell.column.id === "name" ? "shadow-[inset_-1px_0_0_var(--color-zinc-800)]" : ""} ${dim}`}
                    // Ancho exacto (tabla de anchos fijos): si no, la miniatura podía quedar más
                    // estrecha que su hueco y entre ella y el nombre se veía el texto de debajo
                    style={{
                      width: cell.column.getSize(),
                      minWidth: cell.column.getSize(),
                      maxWidth: cell.column.getSize(),
                      left: frozen ? cell.column.getStart() : undefined,
                    }}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                  );
                })}
                <td />
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {nameSort && (
        <LetterBar games={ordered} nameOf={nameText} containerRef={scrollRef} descending={nameSort.desc} />
      )}
    </div>
  );
}
