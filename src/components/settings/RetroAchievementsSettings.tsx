import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";
import { useTranslation } from "../../i18n";
import type { AppSettings } from "../../types";
import { raCheckAccount, type RaProfile } from "../../commands";
import { openExternal, RA_SETTINGS_URL } from "../../utils/links";

/**
 * Ajustes → General → RetroAchievements: usuario y clave de API web, y «Comprobar» (guarda antes
 * lo escrito, porque la comprobación lee los ajustes guardados).
 */
export function RetroAchievementsSettings({ form, setForm }: { form: AppSettings; setForm: (f: AppSettings) => void }) {
  const { t } = useTranslation();
  const updateSettings = useAppStore((s) => s.updateSettings);
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<{ ok: RaProfile } | { error: string } | null>(null);
  const input = "w-full bg-zinc-800 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500";

  const check = async () => {
    setChecking(true);
    setResult(null);
    try {
      await updateSettings(form);
      setResult({ ok: await raCheckAccount() });
    } catch (e) {
      setResult({ error: String(e) });
    } finally {
      setChecking(false);
    }
  };

  return (
    <section>
      <h3 className="text-sm font-medium text-zinc-300 mb-3">RetroAchievements</h3>
      <div className="grid sm:grid-cols-2 gap-2">
        <input
          type="text"
          value={form.ra_user || ""}
          onChange={(e) => setForm({ ...form, ra_user: e.target.value || undefined })}
          placeholder={t("ra.user")}
          autoComplete="off"
          className={input}
        />
        <input
          type="password"
          value={form.ra_apikey || ""}
          onChange={(e) => setForm({ ...form, ra_apikey: e.target.value || undefined })}
          placeholder={t("ra.apiKey")}
          autoComplete="off"
          className={input}
        />
      </div>
      <div className="mt-2 flex items-center gap-3 flex-wrap">
        <button
          onClick={check}
          disabled={checking || !form.ra_user || !form.ra_apikey}
          className="px-3 py-1.5 text-xs text-zinc-200 rounded border border-zinc-600 hover:bg-zinc-700 disabled:opacity-40"
        >
          {checking ? t("ra.checking") : t("ra.check")}
        </button>
        {result && "ok" in result && (
          <span className="flex items-center gap-2 text-xs text-emerald-300">
            {result.ok.user_pic && <img src={result.ok.user_pic} alt="" className="w-5 h-5 rounded" />}
            {t("ra.connected", { user: result.ok.user, points: result.ok.points })}
          </span>
        )}
        {result && "error" in result && <span className="text-xs text-red-300">{result.error}</span>}
      </div>
      <p className="text-xs text-zinc-500 mt-2">
        {t("ra.settingsHelp")}{" "}
        <button onClick={() => openExternal(RA_SETTINGS_URL)} className="text-indigo-300 hover:underline">
          retroachievements.org/settings
        </button>
      </p>
    </section>
  );
}
