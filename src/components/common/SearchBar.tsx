import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";

export function SearchBar() {
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const { t } = useTranslation();

  return (
    <input
      type="text"
      placeholder={t("search.placeholder")}
      value={searchQuery}
      onChange={(e) => setSearchQuery(e.target.value)}
      className="bg-zinc-800 border border-zinc-600 rounded px-3 py-1.5 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500 w-64"
    />
  );
}
