import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Game } from "../../types";
import { useTranslation } from "../../i18n";
import { ImagePreview } from "./ImagePreview";
import { copyImageToSystem, deleteFiles, scanGameMedia } from "../../commands";
import { resolveImageSrc } from "../../utils/images";
import { ExpandIcon, PencilIcon, TrashIcon, CloseIcon, SearchIcon } from "../common/Icons";
import type { TranslationKeys } from "../../i18n/en";

interface ImageManagerProps {
  game: Game;
  systemPath: string;
  onUpdate: (field: keyof Game, value: string) => void;
}

const IMAGE_FIELDS: { key: keyof Game; labelKey: TranslationKeys; subfolder: string }[] = [
  { key: "image", labelKey: "images.boxArt", subfolder: "images" },
  { key: "thumbnail", labelKey: "images.thumbnail", subfolder: "images" },
  { key: "marquee", labelKey: "images.marquee", subfolder: "images" },
];

const EXTENDED_IMAGE_FIELDS: { key: keyof Game; labelKey: TranslationKeys; subfolder: string }[] = [
  { key: "fanart", labelKey: "images.fanart", subfolder: "images" },
  { key: "titleshot", labelKey: "images.titleshot", subfolder: "images" },
  { key: "boxback", labelKey: "images.boxback", subfolder: "images" },
  { key: "boxart", labelKey: "images.boxart", subfolder: "images" },
  { key: "wheel", labelKey: "images.wheel", subfolder: "images" },
  { key: "mix", labelKey: "images.mix", subfolder: "images" },
  { key: "cartridge", labelKey: "images.cartridge", subfolder: "images" },
  { key: "manual", labelKey: "images.manual", subfolder: "images" },
];

export function ImageManager({ game, systemPath, onUpdate }: ImageManagerProps) {
  const { t } = useTranslation();
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

  async function handleChangeImage(field: keyof Game, subfolder: string) {
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

    const sourcePath = String(selected);
    // Extract only the filename stem from the ROM path (no subdirs)
    const romFilename = game.path.replace(/^\.\//, "").split("/").pop() || game.path;
    const baseName = romFilename.replace(/\.[^.]+$/, "");
    const ext = sourcePath.split(".").pop() || "png";
    const filename = `${baseName}-${field}.${ext}`;

    const relativePath = await copyImageToSystem(
      sourcePath,
      systemPath,
      subfolder,
      filename
    );

    onUpdate(field, relativePath);
  }

  async function handleDeleteMedia(field: keyof Game) {
    const mediaPath = game[field] as string | undefined;
    if (!mediaPath) return;

    if (!confirm(t("images.deleteMediaConfirm"))) return;

    // Resolve to absolute path
    const absolutePath = mediaPath.startsWith("/")
      ? mediaPath
      : mediaPath.startsWith("./")
        ? `${systemPath}/${mediaPath.slice(2)}`
        : `${systemPath}/${mediaPath}`;

    try {
      await deleteFiles([absolutePath]);
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

    const sourcePath = String(selected);
    const romFilename = game.path.replace(/^\.\//, "").split("/").pop() || game.path;
    const baseName = romFilename.replace(/\.[^.]+$/, "");
    const ext = sourcePath.split(".").pop() || "mp4";
    const filename = `${baseName}.${ext}`;

    const relativePath = await copyImageToSystem(
      sourcePath,
      systemPath,
      "videos",
      filename
    );

    onUpdate("video", relativePath);
  }

  const videoSrc = resolveImageSrc(game.video, systemPath);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-zinc-300">{t("images.title")}</h4>
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
      <div className="grid grid-cols-3 gap-3">
        {IMAGE_FIELDS.map(({ key, labelKey, subfolder }) => (
          <ImagePreview
            key={key}
            label={t(labelKey)}
            imagePath={game[key] as string | undefined}
            systemPath={systemPath}
            onChangeRequest={() => handleChangeImage(key, subfolder)}
            onDelete={game[key] ? () => handleDeleteMedia(key) : undefined}
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
            {EXTENDED_IMAGE_FIELDS.map(({ key, labelKey, subfolder }) => (
              <ImagePreview
                key={key}
                label={t(labelKey)}
                imagePath={game[key] as string | undefined}
                systemPath={systemPath}
                onChangeRequest={() => handleChangeImage(key, subfolder)}
                onDelete={game[key] ? () => handleDeleteMedia(key) : undefined}
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
