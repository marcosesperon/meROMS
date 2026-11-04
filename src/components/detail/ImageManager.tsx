import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { SWAPPABLE_MEDIA, type Game } from "../../types";
import { useTranslation } from "../../i18n";
import { ImagePreview } from "./ImagePreview";
import { scanGameMedia, setGameMedia, trashMedia } from "../../commands";
import { absoluteMediaPath, resolveImageSrc } from "../../utils/images";
import { ExpandIcon, PencilIcon, TrashIcon, CloseIcon, SearchIcon } from "../common/Icons";
import type { TranslationKeys } from "../../i18n/en";
import { askConfirm } from "../../utils/dialogs";

interface ImageManagerProps {
  game: Game;
  systemPath: string;
  onUpdate: (field: keyof Game, value: string) => void;
  /** Per-game tools in the section header; each button shows only if its handler is given */
  onNormalize?: () => void;
  onClean?: () => void;
  /** "This is...": move a media file to another type (swap if that type already has one) */
  onChangeKind?: (from: keyof Game, to: keyof Game) => void;
}

// The backend decides the folder and file name (unified naming of the active profile)
const IMAGE_FIELDS: { key: keyof Game; labelKey: TranslationKeys }[] = [
  { key: "image", labelKey: "images.boxArt" },
  { key: "thumbnail", labelKey: "images.thumbnail" },
  { key: "marquee", labelKey: "images.marquee" },
];

const EXTENDED_IMAGE_FIELDS: { key: keyof Game; labelKey: TranslationKeys }[] = [
  { key: "fanart", labelKey: "images.fanart" },
  { key: "titleshot", labelKey: "images.titleshot" },
  { key: "boxback", labelKey: "images.boxback" },
  { key: "boxart", labelKey: "images.boxart" },
  { key: "wheel", labelKey: "images.wheel" },
  { key: "mix", labelKey: "images.mix" },
  { key: "cartridge", labelKey: "images.cartridge" },
  { key: "manual", labelKey: "images.manual" },
];

export function ImageManager({ game, systemPath, onUpdate, onNormalize, onClean, onChangeKind }: ImageManagerProps) {
  const { t } = useTranslation();
  // Options of the "This is..." menu for one field (only swappable image types, never itself)
  const kindProps = (field: keyof Game) =>
    onChangeKind && SWAPPABLE_MEDIA.some((m) => m.field === field)
      ? {
          kindOptions: SWAPPABLE_MEDIA.filter((m) => m.field !== field).map((m) => ({ field: m.field, label: t(m.labelKey) })),
          onChangeKind: (to: string) => onChangeKind(field, to as keyof Game),
        }
      : {};
  const [videoFullscreen, setVideoFullscreen] = useState(false);
  const [showExtended, setShowExtended] = useState(false);
  const [scanning, setScanning] = useState(false);

  async function handleScanMedia() {
    setScanning(true);
    try {
      const found = await scanGameMedia(systemPath, game.path);
      const keys = Object.keys(found);
      if (keys.length > 0) {
        for (const [field, path] of Object.entries(found)) {
          // Only update fields that are currently empty
          if (!game[field as keyof Game]) {
            onUpdate(field as keyof Game, path);
          }
        }
      }
    } catch (e) {
      console.error("Scan media failed:", e);
    } finally {
      setScanning(false);
    }
  }

  async function handleChangeImage(field: keyof Game) {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"],
        },
      ],
    });

    if (!selected) return;

    const storedPath = await setGameMedia(systemPath, game.path, field, String(selected));
    onUpdate(field, storedPath);
  }

  async function handleDeleteMedia(field: keyof Game) {
    const mediaPath = game[field] as string | undefined;
    if (!mediaPath) return;

    if (!(await askConfirm(t("images.deleteMediaConfirm")))) return;

    // Resolve to absolute path (also "C:\..." on Windows)
    const absolutePath = absoluteMediaPath(mediaPath, systemPath);

    try {
      // To the trash (recoverable), like every other delete in the app
      await trashMedia(systemPath, [absolutePath]);
      onUpdate(field, "");
    } catch (e) {
      console.error("Failed to delete media:", e);
    }
  }

  async function handleChangeVideo() {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Videos",
          extensions: ["mp4", "mkv", "avi", "mov", "wmv", "webm"],
        },
      ],
    });

    if (!selected) return;

    const storedPath = await setGameMedia(systemPath, game.path, "video", String(selected));
    onUpdate("video", storedPath);
  }

  const videoSrc = resolveImageSrc(game.video, systemPath);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-zinc-300">{t("images.title")}</h4>
        <div className="flex items-center gap-1.5">
          {onNormalize && (
            <button
              onClick={onNormalize}
              className="px-2.5 py-1 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
            >
              {t("gameTools.normalize")}
            </button>
          )}
          {onClean && (
            <button
              onClick={onClean}
              className="px-2.5 py-1 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
            >
              {t("gameTools.clean")}
            </button>
          )}
          <button
            onClick={handleScanMedia}
            disabled={scanning}
            className="flex items-center gap-1.5 px-2.5 py-1 text-xs bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors disabled:opacity-50"
            title={t("images.scanMedia")}
          >
            <SearchIcon size={12} className={scanning ? "animate-spin" : ""} />
            {t("images.scanMedia")}
          </button>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {IMAGE_FIELDS.map(({ key, labelKey }) => (
          <ImagePreview
            key={key}
            label={t(labelKey)}
            imagePath={game[key] as string | undefined}
            systemPath={systemPath}
            onChangeRequest={() => handleChangeImage(key)}
            onDelete={game[key] ? () => handleDeleteMedia(key) : undefined}
            {...kindProps(key)}
          />
        ))}
      </div>

      {/* Extended Media (collapsible) */}
      <div>
        <button
          onClick={() => setShowExtended(!showExtended)}
          className="flex items-center gap-2 text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          <span
            className="inline-block transition-transform"
            style={{ transform: showExtended ? "rotate(90deg)" : "rotate(0deg)" }}
          >
            &#9654;
          </span>
          {t("images.extendedMedia")}
        </button>
        {showExtended && (
          <div className="grid grid-cols-3 gap-3 mt-2">
            {EXTENDED_IMAGE_FIELDS.map(({ key, labelKey }) => (
              <ImagePreview
                key={key}
                label={t(labelKey)}
                imagePath={game[key] as string | undefined}
                systemPath={systemPath}
                onChangeRequest={() => handleChangeImage(key)}
                onDelete={game[key] ? () => handleDeleteMedia(key) : undefined}
                {...kindProps(key)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Video */}
      <div>
        <label className="text-xs text-zinc-400 mb-1 block">
          {t("images.video")}
        </label>
        <div className="relative group w-full aspect-video bg-zinc-800 rounded overflow-hidden border border-zinc-700">
          {videoSrc ? (
            <>
              <video
                src={videoSrc}
                className="w-full h-full object-contain"
                controls
                autoPlay
                loop
                muted
                preload="auto"
              />
              {/* Action buttons for video */}
              <div className="absolute top-2 right-2 flex gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={() => setVideoFullscreen(true)}
                  className="bg-zinc-700/80 hover:bg-zinc-600 text-white p-1.5 rounded transition-colors"
                  title={t("images.fullscreen")}
                >
                  <ExpandIcon size={14} />
                </button>
                <button
                  onClick={handleChangeVideo}
                  className="bg-indigo-600/80 hover:bg-indigo-500 text-white p-1.5 rounded transition-colors"
                  title={t("images.change")}
                >
                  <PencilIcon size={14} />
                </button>
                <button
                  onClick={() => handleDeleteMedia("video")}
                  className="bg-red-600/80 hover:bg-red-500 text-white p-1.5 rounded transition-colors"
                  title={t("images.deleteMedia")}
                >
                  <TrashIcon size={14} />
                </button>
              </div>
            </>
          ) : (
            <button
              onClick={handleChangeVideo}
              className="w-full h-full flex items-center justify-center text-zinc-600 text-sm hover:bg-zinc-700/50 transition-colors cursor-pointer"
            >
              {t("images.noVideo")}
            </button>
          )}
        </div>
      </div>

      {/* Video fullscreen modal */}
      {videoFullscreen && videoSrc && (
        <div
          className="fixed inset-0 z-50 bg-black/90 flex items-center justify-center"
          onClick={() => setVideoFullscreen(false)}
        >
          <video
            src={videoSrc}
            className="max-w-[90vw] max-h-[90vh]"
            controls
            autoPlay
            onClick={(e) => e.stopPropagation()}
          />
          <button
            onClick={() => setVideoFullscreen(false)}
            className="absolute top-4 right-4 text-white bg-black/50 hover:bg-black/80 w-10 h-10 rounded-full flex items-center justify-center transition-colors"
          >
            <CloseIcon size={20} />
          </button>
        </div>
      )}
    </div>
  );
}
