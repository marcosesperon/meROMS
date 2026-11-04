import { useState } from "react";
import { resolveImageSrc } from "../../utils/images";
import { useTranslation } from "../../i18n";
import { ExpandIcon, PencilIcon, TrashIcon, CloseIcon } from "../common/Icons";

interface ImagePreviewProps {
  label: string;
  imagePath?: string;
  systemPath: string;
  onChangeRequest: () => void;
  onDelete?: () => void;
  /** "This is..." menu: other media types this file can be moved to (swapped if taken) */
  kindOptions?: { field: string; label: string }[];
  onChangeKind?: (field: string) => void;
}

export function ImagePreview({
  label,
  imagePath,
  systemPath,
  onChangeRequest,
  onDelete,
  kindOptions,
  onChangeKind,
}: ImagePreviewProps) {
  const src = resolveImageSrc(imagePath, systemPath);
  const { t } = useTranslation();
  const [fullscreen, setFullscreen] = useState(false);

  return (
    <>
      <div>
        <div className="flex items-center justify-between gap-2 mb-1">
          <label className="text-xs text-zinc-400 truncate">{label}</label>
          {/* «Esto es…»: el fichero en realidad es de otro tipo */}
          {src && kindOptions && onChangeKind && (
            <select
              value=""
              onChange={(e) => e.target.value && onChangeKind(e.target.value)}
              className="max-w-[90px] bg-transparent text-[10px] text-zinc-500 hover:text-zinc-300 focus:outline-none cursor-pointer"
              title={t("mediaKinds.thisIs")}
            >
              <option value="">{t("mediaKinds.thisIs")}</option>
              {kindOptions.map((o) => (
                <option key={o.field} value={o.field}>
                  {o.label}
                </option>
              ))}
            </select>
          )}
        </div>
        <div className="relative group w-full aspect-video bg-zinc-800 rounded overflow-hidden border border-zinc-700">
          {src ? (
            <>
              <img
                src={src}
                alt={label}
                className="w-full h-full object-contain"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
              {/* Action buttons overlay */}
              <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                <button
                  onClick={() => setFullscreen(true)}
                  className="bg-zinc-700/80 hover:bg-zinc-600 text-white p-2 rounded transition-colors"
                  title={t("images.fullscreen")}
                >
                  <ExpandIcon size={14} />
                </button>
                <button
                  onClick={onChangeRequest}
                  className="bg-indigo-600/80 hover:bg-indigo-500 text-white p-2 rounded transition-colors"
                  title={t("images.change")}
                >
                  <PencilIcon size={14} />
                </button>
                {onDelete && (
                  <button
                    onClick={onDelete}
                    className="bg-red-600/80 hover:bg-red-500 text-white p-2 rounded transition-colors"
                    title={t("images.deleteMedia")}
                  >
                    <TrashIcon size={14} />
                  </button>
                )}
              </div>
            </>
          ) : (
            <button
              onClick={onChangeRequest}
              className="w-full h-full flex items-center justify-center text-zinc-600 text-sm hover:bg-zinc-700/50 transition-colors cursor-pointer"
            >
              {t("images.noImage")}
            </button>
          )}
        </div>
      </div>

      {/* Fullscreen modal */}
      {fullscreen && src && (
        <div
          className="fixed inset-0 z-50 bg-black/90 flex items-center justify-center cursor-pointer"
          onClick={() => setFullscreen(false)}
        >
          <img
            src={src}
            alt={label}
            className="max-w-[90vw] max-h-[90vh] object-contain"
            onClick={(e) => e.stopPropagation()}
          />
          <button
            onClick={() => setFullscreen(false)}
            className="absolute top-4 right-4 text-white bg-black/50 hover:bg-black/80 w-10 h-10 rounded-full flex items-center justify-center transition-colors"
          >
            <CloseIcon size={20} />
          </button>
        </div>
      )}
    </>
  );
}
