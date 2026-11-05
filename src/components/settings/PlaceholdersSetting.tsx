import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n";
import { forgetPlaceholders, placeholderCount } from "../../commands";
import { askConfirm } from "../../utils/dialogs";

/**
 * «N rellenos aprendidos» con el botón Olvidar. Los rellenos se aprenden en «Limpiar media»
 * y el scraper los ignora; esto es por si se apuntó uno por error.
 */
export function PlaceholdersSetting() {
  const { t } = useTranslation();
  const [count, setCount] = useState<number | null>(null);

  useEffect(() => {
    placeholderCount().then(setCount).catch(() => setCount(null));
  }, []);

  async function forget() {
    if (!(await askConfirm(t("placeholders.forgetConfirm", { count: count ?? 0 })))) return;
    await forgetPlaceholders();
    setCount(0);
  }

  if (count === null) return null;

  return (
    <div className="mt-3 flex items-center gap-3 text-xs">
      <p className="flex-1 text-zinc-400">{t("placeholders.learned", { count })}</p>
      {count > 0 && (
        <button onClick={forget} className="px-3 py-1.5 bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600">
          {t("placeholders.forget")}
        </button>
      )}
    </div>
  );
}
