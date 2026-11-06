import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "../../i18n";
import type { Game } from "../../types";
import { datGameInfo, gameFiles, gameHashes, rawGameFields, type DatMatch, type GameFiles, type GameHashes } from "../../commands";
import { formatBytes } from "../../utils/format";
import { openExternal } from "../../utils/links";
import { CheckIcon, CopyIcon } from "../common/Icons";

// Fuentes de metadatos de meROMS y la ficha de cada una por su id. La gamelist solo guarda
// un id (atributos `id` y `source` del <game>): el de ScreenScraper casi siempre.
const SOURCES: { name: string; match: RegExp; url?: (id: string) => string }[] = [
  { name: "ScreenScraper", match: /screenscraper/i, url: (id) => `https://www.screenscraper.fr/gameinfos.php?gameid=${encodeURIComponent(id)}` },
  { name: "TheGamesDB", match: /thegamesdb|tgdb/i, url: (id) => `https://thegamesdb.net/game.php?id=${encodeURIComponent(id)}` },
  { name: "LaunchBox", match: /launchbox/i },
  { name: "ArcadeItalia", match: /arcadeitalia|adb/i },
];

/**
 * Pestaña «Metadatos» de la ficha a pantalla completa: ficheros, huellas (calculadas al
 * abrirla), lo que dice el DAT, las fuentes y los campos del gamelist.xml tal cual.
 */
export function MetadataTab({ game, systemPath }: { game: Game; systemPath: string }) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<GameFiles | null>(null);
  // undefined = calculando; null = sin huella (una ROM carpeta)
  const [hashes, setHashes] = useState<GameHashes | null | undefined>(undefined);
  const [hashError, setHashError] = useState<string | null>(null);
  const [dat, setDat] = useState<DatMatch | null | undefined>(undefined);
  const [fields, setFields] = useState<[string, string][]>([]);

  useEffect(() => {
    let cancelled = false;
    setFiles(null);
    setHashes(undefined);
    setHashError(null);
    setDat(undefined);
    setFields([]);
    gameFiles(systemPath, game.path).then((f) => !cancelled && setFiles(f)).catch(() => {});
    gameHashes(systemPath, game.path)
      .then((h) => !cancelled && setHashes(h))
      .catch((e) => !cancelled && setHashError(String(e)));
    datGameInfo(systemPath, game.path).then((d) => !cancelled && setDat(d)).catch(() => !cancelled && setDat(null));
    rawGameFields(systemPath, game.path).then((f) => !cancelled && setFields(f)).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [game.path, systemPath]);

  // Fuente de cada id: la que dice `source`; sin `source`, ScreenScraper (lo habitual)
  const gameId = game["@id"];
  const source = game["@source"] ?? "";
  const linked = gameId ? SOURCES.find((s) => s.match.test(source)) ?? (source ? undefined : SOURCES[0]) : undefined;

  return (
    <div className="h-full overflow-y-auto pr-2 space-y-6">
      <Section title={t("meta.file")}>
        {!files ? (
          <p className="text-sm text-zinc-500">…</p>
        ) : !files.exists ? (
          <p className="text-sm text-red-300">{t("meta.missing")}</p>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-4">
              <Field label={t("meta.fileName")} value={files.name} />
              <Field label={t("meta.size")} value={formatBytes(files.size)} />
            </div>
            {files.files.length > 1 && (
              <ul className="mt-3 space-y-0.5 text-xs text-zinc-400 font-mono">
                {files.files.map((f) => (
                  <li key={f.path} className="flex gap-3">
                    <span className="flex-1 truncate" title={f.path}>{f.path}</span>
                    <span className="shrink-0 text-zinc-500">{formatBytes(f.size)}</span>
                  </li>
                ))}
              </ul>
            )}
            {files.archive && (
              <>
                <p className="mt-3 text-[11px] uppercase tracking-wider text-zinc-500">{t("meta.inside")}</p>
                <ul className="mt-1 space-y-0.5 text-xs text-zinc-400 font-mono">
                  {files.archive.map((e) => (
                    <li key={e.name} className="flex gap-3">
                      <span className="flex-1 truncate" title={e.name}>{e.name}</span>
                      <span className="shrink-0 text-zinc-500">{e.crc}</span>
                      <span className="shrink-0 w-16 text-right text-zinc-500">{formatBytes(e.size)}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </>
        )}
      </Section>

      <Section title={t("meta.hashes")}>
        {hashError ? (
          <p className="text-sm text-red-300">{hashError}</p>
        ) : hashes === undefined ? (
          <p className="text-sm text-zinc-500">{t("meta.hashing")}</p>
        ) : hashes === null ? (
          <p className="text-sm text-zinc-500">{t("meta.noHashes")}</p>
        ) : (
          <>
            <div className="flex flex-wrap gap-2">
              <HashChip label="CRC" value={hashes.crc} />
              <HashChip label="MD5" value={hashes.md5} />
              <HashChip label="SHA-1" value={hashes.sha1} />
            </div>
            <p className="mt-2 text-[11px] text-zinc-500">
              {hashes.inside_archive ? t("meta.hashedInside", { name: hashes.target }) : t("meta.hashedFile", { name: hashes.target })} · {formatBytes(hashes.size)}
            </p>
          </>
        )}
      </Section>

      <Section title={t("meta.verification")}>
        {dat === undefined ? (
          <p className="text-sm text-zinc-500">…</p>
        ) : dat === null ? (
          <p className="text-sm text-zinc-500">{t("meta.noDat")}</p>
        ) : (
          <div className="space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className={`px-3 py-1 rounded-md border text-sm ${dat.by ? "border-emerald-500/50 text-emerald-300" : "border-zinc-600 text-zinc-400"}`}>
                {dat.dat}
              </span>
              <span className="text-sm text-zinc-300">
                {dat.by === "content" ? t("meta.datByContent") : dat.by === "name" ? t("meta.datByName") : t("meta.datNotFound")}
              </span>
            </div>
            {dat.name && (
              <p className="text-sm text-zinc-200">
                {dat.description || dat.name}
                {dat.description && <span className="text-zinc-500 font-mono text-xs"> · {dat.name}</span>}
              </p>
            )}
            {(dat.cloneof || dat.bios) && (
              <p className="text-xs text-amber-300">{dat.bios ? "BIOS" : t("meta.cloneOf", { name: dat.cloneof ?? "" })}</p>
            )}
          </div>
        )}
      </Section>

      <Section title={t("meta.sources")}>
        <div className="grid grid-cols-2 xl:grid-cols-4 gap-2">
          {SOURCES.map((s) => {
            const id = linked === s ? gameId : undefined;
            return (
              <div key={s.name} className={`p-3 rounded-md border ${id ? "border-zinc-600 bg-zinc-900/80" : "border-zinc-800 bg-zinc-900/40"}`}>
                <div className="flex items-center gap-2">
                  <span className={`flex-1 text-sm font-medium ${id ? "text-zinc-100" : "text-zinc-500"}`}>{s.name}</span>
                  {id && s.url && (
                    <button onClick={() => openExternal(s.url!(id))} title={t("meta.openSource")} className="text-xs text-indigo-300 hover:text-indigo-200">
                      ↗
                    </button>
                  )}
                </div>
                <p className={`mt-1 text-sm ${id ? "text-zinc-300 font-mono" : "text-zinc-600 italic"}`}>{id ?? t("meta.notLinked")}</p>
              </div>
            );
          })}
        </div>
      </Section>

      <Section title={t("meta.gamelistFields")}>
        {fields.length === 0 ? (
          <p className="text-sm text-zinc-500">…</p>
        ) : (
          <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-xs">
            {fields.map(([key, value], i) => (
              <div key={`${key}-${i}`} className="contents">
                <dt className="text-zinc-500 font-mono">{key}</dt>
                <dd className="text-zinc-300 font-mono break-all line-clamp-3" title={value}>{value || "—"}</dd>
              </div>
            ))}
          </dl>
        )}
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="text-sm font-semibold text-zinc-200 mb-2">{title}</h3>
      {children}
    </section>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] uppercase tracking-wider text-zinc-500">{label}</p>
      <p className="text-sm text-zinc-200 truncate" title={value}>{value}</p>
    </div>
  );
}

/** Una huella con su botón de copiar (enseña el principio y el final si es larga) */
function HashChip({ label, value }: { label: string; value: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const short = value.length > 16 ? `${value.slice(0, 6)}…${value.slice(-6)}` : value;
  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      // Sin portapapeles: no pasa nada
    }
  }
  return (
    <button
      onClick={copy}
      title={`${value} — ${t("meta.copy")}`}
      className="flex items-center gap-2 px-3 py-1.5 rounded-md border border-zinc-700 bg-zinc-900/80 hover:border-zinc-500 font-mono text-xs"
    >
      <span className="text-zinc-500 font-semibold">{label}</span>
      <span className="text-zinc-200">{short}</span>
      {copied ? <CheckIcon size={12} className="text-emerald-400" /> : <CopyIcon size={12} className="text-zinc-500" />}
    </button>
  );
}
