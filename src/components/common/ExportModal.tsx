import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../../i18n";
import { exportGamelist } from "../../commands";

interface ExportModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
}

const FORMATS = [{ value: "retroarch", label: "RetroArch (.lpl)" }];

export function ExportModal({
  open: isOpen,
  systemPath,
  onClose,
}: ExportModalProps) {
  const { t } = useTranslation();
  const [format, setFormat] = useState("retroarch");
  const [outputDir, setOutputDir] = useState("");
  const [result, setResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

  if (!isOpen) return null;

  async function handleSelectDir() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      setOutputDir(typeof selected === "string" ? selected : selected);
      setResult(null);
      setError(null);
    }
  }

  async function handleExport() {
    if (!outputDir) {
      setError(t("export.noDir"));
      return;
    }
    setExporting(true);
    setError(null);
    setResult(null);
    try {
      const filename = await exportGamelist(systemPath, format, outputDir);
      setResult(t("export.success").replace("{file}", filename));
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl w-full max-w-md p-6">
        <h2 className="text-lg font-semibold text-zinc-100 mb-4">
          {t("export.title")}
        </h2>

        {/* Format */}
        <div className="mb-4">
          <label className="block text-sm text-zinc-400 mb-1">
            {t("export.format")}
          </label>
          <select
            value={format}
            onChange={(e) => setFormat(e.target.value)}
            className="w-full bg-zinc-900 border border-zinc-600 text-zinc-200 rounded px-3 py-2 text-sm"
          >
            {FORMATS.map((f) => (
              <option key={f.value} value={f.value}>
                {f.label}
              </option>
            ))}
          </select>
        </div>

        {/* Output directory */}
        <div className="mb-4">
          <label className="block text-sm text-zinc-400 mb-1">
            {t("export.outputDir")}
          </label>
          <div className="flex gap-2">
            <input
              type="text"
              readOnly
              value={outputDir}
              placeholder={t("export.selectDir")}
              className="flex-1 bg-zinc-900 border border-zinc-600 text-zinc-200 rounded px-3 py-2 text-sm"
            />
            <button
              onClick={handleSelectDir}
              className="px-3 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
            >
              {t("export.selectDir")}
            </button>
          </div>
        </div>

        {/* Result / Error */}
        {result && (
          <div className="mb-4 p-3 bg-emerald-900/30 border border-emerald-700 rounded text-emerald-300 text-sm">
            {result}
          </div>
        )}
        {error && (
          <div className="mb-4 p-3 bg-red-900/30 border border-red-700 rounded text-red-300 text-sm">
            {error}
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("cleanup.close")}
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50"
          >
            {exporting ? "..." : t("export.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
