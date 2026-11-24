import { gameHashes, trashFiles } from "../commands";
import type { TranslationKeys } from "../i18n/en";
import { askChoice } from "../components/common/ChoiceDialog";
import { translate } from "../i18n";
import { useAppStore } from "../store/useAppStore";
import type { Game } from "../types";
import { removeVersions } from "./collectionActions";
import { collectionRulesFrom } from "./collectionRules";
import { formatBytes } from "./format";
import { isPlayed } from "./playStats";

/** Qué ha decidido el usuario cuando el nombre correcto ya lo tiene otra ROM */
export type ClashChoice = "keep" | "removeThis" | "removeOther";

/** Ficha de una de las dos ROMs: título, fichero, tamaño y marcas */
function RomCard({ label, game, file, size, same, recommended }: { label: string; game: Game | undefined; file: string; size: number | null; same: boolean; recommended?: boolean }) {
  const badges = [
    game?.favorite === "true" && translate("clash.favorite"),
    game && isPlayed(game) && translate("clash.played"),
    !game && translate("clash.notInGamelist"),
  ].filter(Boolean) as string[];
  return (
    <div className={`flex-1 min-w-0 rounded border p-3 ${same ? "border-emerald-700/60" : "border-zinc-700"} bg-zinc-900/50`}>
      <p className="text-[11px] uppercase tracking-wider text-zinc-500 font-semibold">{label}</p>
      {recommended && <p className="mt-0.5 text-[10px] text-emerald-300">{translate("clash.recommended")}</p>}
      <p className="mt-1 text-sm text-zinc-100 truncate">{game?.name || "—"}</p>
      <p className="text-xs text-zinc-400 font-mono truncate" title={file}>
        {file}
      </p>
      {size !== null && <p className="text-xs text-zinc-500 mt-0.5">{formatBytes(size)}</p>}
      {badges.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          {badges.map((b) => (
            <span key={b} className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-700 text-zinc-300">
              {b}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Tras scrapear, el nombre correcto de la ROM ya lo tiene otra (en la gamelist o como fichero
 * suelto): se enseñan las dos (y si son la misma ROM, por su contenido) y se pregunta qué hacer.
 * - Mantener las dos: el fichero no se renombra (el título sí se puede cambiar).
 * - Eliminar esta: la recién scrapeada, con su ROM y medios, a la papelera o a apartados.
 * - Eliminar la otra y renombrar esta: la otra fuera (con sus medios si está en la gamelist; si
 *   es un fichero suelto, a la papelera) y esta se queda con el nombre correcto.
 * Hace lo de «eliminar»; renombrar lo hace quien llama. Devuelve lo elegido.
 */
export async function resolveRenameClash(
  systemPath: string,
  thisPath: string,
  otherPath: string,
  /** Es el mismo juego que otro de la colección (Versiones y duplicados): el motivo y cuál se queda */
  duplicate?: { reason: string; detail: string | null; keep: string }
): Promise<ClashChoice> {
  const { games } = useAppStore.getState();
  const mine = games.find((g) => g.path === thisPath);
  const other = games.find((g) => g.path === otherPath);
  const [a, b] = await Promise.all([gameHashes(systemPath, thisPath).catch(() => null), gameHashes(systemPath, otherPath).catch(() => null)]);
  const same = !!a && !!b && a.md5 === b.md5;
  const name = (p: string) => p.replace(/^.*\//, "");

  const choice = (await askChoice({
    title: translate("clash.title"),
    body: (
      <div className="space-y-3">
        <p>
          {duplicate
            ? translate("clash.helpDuplicate", { reason: translate(`duplicates.reason_${duplicate.reason}` as TranslationKeys, { id: duplicate.detail ?? "" }) })
            : translate("clash.help", { name: name(otherPath) })}
        </p>
        {same && <p className="text-emerald-300">{translate("clash.same")}</p>}
        <div className="flex gap-3">
          <RomCard label={translate("clash.this")} game={mine} file={name(thisPath)} size={a?.size ?? null} same={same} recommended={duplicate?.keep === thisPath} />
          <RomCard label={translate("clash.other")} game={other} file={name(otherPath)} size={b?.size ?? null} same={same} recommended={duplicate?.keep === otherPath} />
        </div>
      </div>
    ),
    options: [
      { id: "keep", label: translate("clash.keep") },
      { id: "removeThis", label: translate("clash.removeThis"), danger: true },
      { id: "removeOther", label: translate(duplicate ? "clash.removeOtherKeep" : "clash.removeOther"), primary: true },
    ],
  })) as ClashChoice;

  if (choice === "keep") return choice;
  // Como las versiones que sobran: apartar si así lo dice «Mi colección» (ocultar no es quitar)
  const rule = collectionRulesFrom(useAppStore.getState().settings.collection_rules).versions_action;
  const action = rule === "set_aside" ? "set_aside" : "trash";
  const gone = choice === "removeThis" ? thisPath : otherPath;
  if (games.some((g) => g.path === gone)) {
    // Sin carpeta de apartados elegida, apartar falla: entonces, a la papelera
    await removeVersions(systemPath, [gone], action).catch(() => removeVersions(systemPath, [gone], "trash"));
  } else {
    // Un fichero suelto con ese nombre (no está en la gamelist): a la papelera
    await trashFiles(systemPath, [`${systemPath}/${gone.replace(/^\.\//, "")}`]);
  }
  await useAppStore.getState().loadGames(systemPath);
  return choice;
}
