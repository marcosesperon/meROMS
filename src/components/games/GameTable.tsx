import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
  type ColumnResizeMode,
} from "@tanstack/react-table";
import { useMemo, useRef, useState } from "react";
import { useListKeyboard } from "../../hooks/useListKeyboard";
import type { Game } from "../../types";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { StarIcon, StarOutlineIcon, EyeSlashIcon, ChildIcon } from "../common/Icons";
import { getFlag } from "../../utils/flags";
import { resolveImageSrc } from "../../utils/images";
import { DatTag } from "../common/DatTag";

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

// Columnas «congeladas» en el scroll horizontal: la miniatura y el nombre
const FROZEN = new Set(["thumb", "name"]);
// Fondo opaco de las celdas congeladas (si fuera transparente, se verían las columnas que
// pasan por debajo): el mismo color que la fila, ya mezclado con el fondo
const FROZEN_BG = {
  normal: "bg-zinc-900 group-hover:bg-[color-mix(in_srgb,var(--color-zinc-800)_50%,var(--color-zinc-900))]",
  selected: "bg-[color-mix(in_srgb,var(--color-indigo-600)_15%,var(--color-zinc-900))]",
};

export function GameTable({ games }: GameTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnResizeMode] = useState<ColumnResizeMode>("onChange");
  const selectGame = useAppStore((s) => s.selectGame);
  const openEditor = useAppStore((s) => s.openEditor);
  const missingRomPaths = useAppStore((s) => s.missingRomPaths);
  const multiSelect = useAppStore((s) => s.multiSelect);
  const selectedPaths = useAppStore((s) => s.selectedPaths);
  const toggleSelected = useAppStore((s) => s.toggleSelected);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const { t } = useTranslation();

  const columns = useMemo(
    () => [
      // Miniatura: el mismo medio que la cuadrícula (Ajustes → General), en pequeño
      columnHelper.display({
        id: "thumb",
        header: "",
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
        size: 100,
        minSize: 80,
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
    ],
    [t]
  );

  const table = useReactTable({
    data: games,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    columnResizeMode,
    enableColumnResizing: true,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  // Teclado: las filas en el orden en que se ven (con la columna de orden elegida)
  const scrollRef = useRef<HTMLDivElement>(null);
  useListKeyboard({
    items: table.getRowModel().rows.map((r) => r.original),
    containerRef: scrollRef,
    getColumns: () => 1,
  });

  return (
    <div ref={scrollRef} className="overflow-auto flex-1">
      <table
        className="text-sm border-collapse table-fixed"
        style={{ width: table.getCenterTotalSize() }}
      >
        <thead className="sticky top-0 bg-zinc-800 z-10">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <th
                  key={header.id}
                  className={`text-left px-4 py-2.5 text-zinc-400 font-medium border-b border-zinc-700 select-none ${
                    FROZEN.has(header.column.id) ? "sticky z-20 bg-zinc-800" : "relative"
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
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr
              key={row.id}
              data-game-path={row.original.path}
              onClick={() => (multiSelect ? toggleSelected(row.original.path) : selectGame(row.original))}
              onDoubleClick={() => {
                if (multiSelect) return;
                selectGame(row.original);
                openEditor();
              }}
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
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
