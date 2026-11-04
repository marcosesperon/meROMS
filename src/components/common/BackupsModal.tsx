import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { listBackups, restoreBackup, deleteBackup } from "../../commands";
import { useAppStore } from "../../store/useAppStore";
import type { BackupEntry } from "../../types";
import { askConfirm, showMessage } from "../../utils/dialogs";

interface BackupsModalProps {
  open: boolean;
  systemPath: string;
  onClose: () => void;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function BackupsModal({ open, systemPath, onClose }: BackupsModalProps) {
  const { t } = useTranslation();
  const loadGames = useAppStore((s) => s.loadGames);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open) {
      setBackups([]);
      return;
    }
    loadBackups();
  }, [open, systemPath]);

  async function loadBackups() {
    setLoading(true);
    try {
      const result = await listBackups(systemPath);
      setBackups(result);
    } catch (e) {
      console.error("Failed to list backups:", e);
    } finally {
      setLoading(false);
    }
  }

  async function handleRestore(filename: string) {
    if (!(await askConfirm(t("backup.confirmRestore")))) return;
    try {
      await restoreBackup(systemPath, filename);
      loadGames(systemPath);
      await showMessage(t("backup.restored"));
      onClose();
    } catch (e) {
      await showMessage(t("backup.restoreFailed", { error: String(e) }), "error");
    }
  }

  async function handleDelete(filename: string) {
    try {
      await deleteBackup(systemPath, filename);
      setBackups((prev) => prev.filter((b) => b.filename !== filename));
    } catch (e) {
      console.error("Failed to delete backup:", e);
    }
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[560px] max-h-[80vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">
            {t("backup.title")}
          </h3>
        </div>

        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center py-12 text-zinc-500">
              ...
            </div>
          ) : backups.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-zinc-500 text-sm px-8 text-center">
              {t("backup.noBackups")}
            </div>
          ) : (
            <div className="divide-y divide-zinc-700/30">
              {backups.map((backup) => (
                <div
                  key={backup.filename}
                  className="flex items-center gap-3 px-5 py-3 hover:bg-zinc-700/20"
                >
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-zinc-200">{backup.timestamp}</p>
                    <p className="text-xs text-zinc-500">
                      {formatSize(backup.size)}
                    </p>
                  </div>
                  <button
                    onClick={() => handleRestore(backup.filename)}
                    className="px-3 py-1.5 text-xs bg-indigo-600/20 text-indigo-400 rounded hover:bg-indigo-600/30 transition-colors"
                  >
                    {t("backup.restore")}
                  </button>
                  <button
                    onClick={() => handleDelete(backup.filename)}
                    className="px-3 py-1.5 text-xs bg-red-600/20 text-red-400 rounded hover:bg-red-600/30 transition-colors"
                  >
                    {t("backup.delete")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors"
          >
            {t("backup.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
