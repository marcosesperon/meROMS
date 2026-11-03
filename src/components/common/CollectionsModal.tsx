import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import {
  detectMultidiscGames,
  generateM3uPlaylists,
  generateCollection,
} from "../../commands";
import type { DiscGroup } from "../../commands";
import type { Game } from "../../types";

interface CollectionsModalProps {
  open: boolean;
  systemPath: string;
  games: Game[];
  onClose: () => void;
}

type Tab = "m3u" | "collection";

export function CollectionsModal({
  open,
  systemPath,
  games,
  onClose,
}: CollectionsModalProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<Tab>("m3u");

  // M3U state
  const [discGroups, setDiscGroups] = useState<DiscGroup[]>([]);
  const [detecting, setDetecting] = useState(false);
  const [m3uResult, setM3uResult] = useState<string[] | null>(null);

  // Collection state
  const [collectionName, setCollectionName] = useState("");
  const [selectedGames, setSelectedGames] = useState<Set<string>>(new Set());
  const [collectionResult, setCollectionResult] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setDiscGroups([]);
      setM3uResult(null);
      setCollectionName("");
      setSelectedGames(new Set());
      setCollectionResult(null);
      return;
    }
    detectDiscs();
  }, [open, systemPath]);

  async function detectDiscs() {
    setDetecting(true);
    try {
      const result = await detectMultidiscGames(systemPath);
      setDiscGroups(result);
    } catch (e) {
      console.error("Multi-disc detection failed:", e);
    } finally {
      setDetecting(false);
    }
  }

  async function handleGenerateM3u() {
    try {
      const created = await generateM3uPlaylists(systemPath, discGroups);
      setM3uResult(created);
    } catch (e) {
      console.error("M3U generation failed:", e);
    }
  }

  function toggleGame(path: string) {
    setSelectedGames((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  function toggleAllGames() {
    if (selectedGames.size === games.length) {
      setSelectedGames(new Set());
    } else {
      setSelectedGames(new Set(games.map((g) => g.path)));
    }
  }

  async function handleGenerateCollection() {
    if (!collectionName.trim() || selectedGames.size === 0) return;
    try {
      const filename = await generateCollection(
        systemPath,
        collectionName.trim(),
        Array.from(selectedGames)
      );
      setCollectionResult(filename);
    } catch (e) {
      console.error("Collection generation failed:", e);
    }
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[680px] max-h-[80vh] flex flex-col mx-4">
        {/* Header */}
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {t("collections.title")}
          </h3>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-zinc-700">
          <button
            onClick={() => setActiveTab("m3u")}
            className={`flex-1 px-4 py-2.5 text-sm font-medium transition-colors ${
              activeTab === "m3u"
                ? "text-indigo-400 border-b-2 border-indigo-400"
                : "text-zinc-400 hover:text-zinc-300"
            }`}
          >
            {t("collections.m3uTab")}
          </button>
          <button
            onClick={() => setActiveTab("collection")}
            className={`flex-1 px-4 py-2.5 text-sm font-medium transition-colors ${
              activeTab === "collection"
                ? "text-indigo-400 border-b-2 border-indigo-400"
                : "text-zinc-400 hover:text-zinc-300"
            }`}
          >
            {t("collections.collectionTab")}
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {activeTab === "m3u" ? (
            <div className="p-5">
              {detecting ? (
                <div className="flex items-center justify-center py-12 text-zinc-500">
                  {t("collections.detecting")}
                </div>
              ) : m3uResult ? (
                <div className="text-sm text-emerald-400">
                  {t("collections.generated", { files: m3uResult.join(", ") })}
                </div>
              ) : discGroups.length === 0 ? (
                <div className="flex items-center justify-center py-12 text-zinc-500 text-sm">
                  {t("collections.noMultidisc")}
                </div>
              ) : (
                <div className="space-y-4">
                  {discGroups.map((group, i) => (
                    <div key={i} className="bg-zinc-900/50 rounded-lg p-3">
                      <div className="text-sm font-medium text-zinc-200 mb-2">
                        {group.base_name}
                      </div>
                      <div className="space-y-1">
                        {group.files.map((file, fi) => (
                          <div
                            key={fi}
                            className="text-xs text-zinc-400 pl-3"
                          >
                            {file}
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="p-5 space-y-4">
              {collectionResult ? (
                <div className="text-sm text-emerald-400">
                  {t("collections.generated", { files: collectionResult })}
                </div>
              ) : (
                <>
                  <div>
                    <label className="block text-sm text-zinc-400 mb-1">
                      {t("collections.collectionName")}
                    </label>
                    <input
                      type="text"
                      value={collectionName}
                      onChange={(e) => setCollectionName(e.target.value)}
                      placeholder="My Collection"
                      className="w-full px-3 py-2 bg-zinc-900 border border-zinc-600 rounded text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                    />
                  </div>
                  <div>
                    <div className="flex items-center justify-between mb-2">
                      <label className="text-sm text-zinc-400">
                        {t("collections.selectGames")}
                      </label>
                      <button
                        onClick={toggleAllGames}
                        className="text-xs text-indigo-400 hover:text-indigo-300"
                      >
                        {selectedGames.size === games.length
                          ? t("cleanup.deselectAll")
                          : t("cleanup.selectAll")}
                      </button>
                    </div>
                    <div className="max-h-60 overflow-y-auto border border-zinc-700 rounded divide-y divide-zinc-700/30">
                      {games.map((game) => (
                        <label
                          key={game.path}
                          className="flex items-center gap-3 px-3 py-1.5 hover:bg-zinc-700/30 cursor-pointer transition-colors"
                        >
                          <input
                            type="checkbox"
                            checked={selectedGames.has(game.path)}
                            onChange={() => toggleGame(game.path)}
                            className="rounded border-zinc-600 bg-zinc-700 text-indigo-600 shrink-0"
                          />
                          <span className="text-sm text-zinc-300 truncate">
                            {game.name || game.path}
                          </span>
                        </label>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("cleanup.close")}
          </button>
          {activeTab === "m3u" && discGroups.length > 0 && !m3uResult && (
            <button
              onClick={handleGenerateM3u}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
            >
              {t("collections.generate")}
            </button>
          )}
          {activeTab === "collection" && !collectionResult && (
            <button
              onClick={handleGenerateCollection}
              disabled={!collectionName.trim() || selectedGames.size === 0}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t("collections.generate")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
