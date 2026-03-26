import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { ViewMode } from "../../types";

export function ViewToggle() {
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const { t } = useTranslation();

  const modes: { mode: ViewMode; label: string; icon: string }[] = [
    { mode: "table", label: t("view.table"), icon: "☰" },
    { mode: "grid", label: t("view.grid"), icon: "⊞" },
  ];

  return (
    <div className="flex border border-zinc-600 rounded overflow-hidden">
      {modes.map(({ mode, label, icon }) => (
        <button
          key={mode}
          onClick={() => setViewMode(mode)}
          title={label}
          className={`px-2.5 py-1.5 text-base flex items-center justify-center transition-colors ${
            viewMode === mode
              ? "bg-indigo-600 text-white"
              : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
          }`}
        >
          <span>{icon}</span>
        </button>
      ))}
    </div>
  );
}
