import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
  type ColumnResizeMode,
} from "@tanstack/react-table";
import { useMemo, useState } from "react";
import type { Game } from "../../types";
import { formatRating, formatReleaseDate } from "../../utils/format";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import { StarIcon, StarOutlineIcon, EyeSlashIcon, ChildIcon } from "../common/Icons";
import { getFlag } from "../../utils/flags";

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

export function GameTable({ games }: GameTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnResizeMode] = useState<ColumnResizeMode>("onChange");
  const selectGame = useAppStore((s) => s.selectGame);
  const openEditor = useAppStore((s) => s.openEditor);
  const selectedGame = useAppStore((s) => s.selectedGame);
  const { t } = useTranslation();

  const columns = useMemo(
    () => [
      columnHelper.accessor("name", {
        header: t("table.name"),
        cell: (info) => info.getValue() || info.row.original.path,
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

  return (
    <div className="overflow-auto flex-1">
      <table
        className="text-sm border-collapse"
        style={{ width: table.getCenterTotalSize() }}
      >
        <thead className="sticky top-0 bg-zinc-800 z-10">
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <th
                  key={header.id}
                  className="relative text-left px-4 py-2.5 text-zinc-400 font-medium border-b border-zinc-700 select-none"
                  style={{ width: header.getSize() }}
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
              onClick={() => selectGame(row.original)}
              onDoubleClick={() => {
                selectGame(row.original);
                openEditor();
              }}
              className={`cursor-pointer border-b border-zinc-800 transition-colors ${
                selectedGame?.path === row.original.path
                  ? "bg-indigo-600/15"
                  : "hover:bg-zinc-800/50"
              }`}
            >
              {row.getVisibleCells().map((cell) => (
                <td
                  key={cell.id}
                  className="px-4 py-2.5 text-zinc-300 truncate"
                  style={{ width: cell.column.getSize() }}
                >
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
