import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "../../i18n";
import { AUTHOR, DONATION_URL, REPOSITORY_URL, WEBSITE_URL, openExternal } from "../../utils/links";
import { CoffeeIcon, GitHubIcon, GlobeIcon } from "../common/Icons";
import appIcon from "../../assets/app-icon.png";
import { useUpdateStore } from "../../store/useUpdateStore";

interface AboutTabProps {
  /** «Buscar actualizaciones automáticamente» (se guarda con el resto de Ajustes) */
  checkUpdates: boolean;
  onCheckUpdatesChange: (on: boolean) => void;
}

/** Pestaña «Acerca de» de Ajustes: versión, autor, enlaces (repositorio, web y donativo) y actualizaciones */
export function AboutTab({ checkUpdates, onCheckUpdatesChange }: AboutTabProps) {
  const { t } = useTranslation();
  const updatePhase = useUpdateStore((s) => s.phase);
  const update = useUpdateStore((s) => s.update);
  const updateError = useUpdateStore((s) => s.error);
  const checkNow = useUpdateStore((s) => s.checkNow);
  const openUpdate = useUpdateStore((s) => s.openModal);
  // La versión se lee de la propia app (tauri.conf.json), así no hay que tocarla aquí
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  const link = "inline-flex items-center gap-2 px-4 py-2 text-sm rounded transition-colors";

  return (
    <div className="max-w-xl space-y-6">
      <div className="flex items-center gap-4">
        <img src={appIcon} alt="" className="w-16 h-16 rounded-2xl" />
        <div>
          <h3 className="text-xl font-semibold text-zinc-100">meROMS {version && <span className="text-zinc-500 font-normal">v{version}</span>}</h3>
          <p className="text-sm text-zinc-400">{t("about.madeBy", { author: AUTHOR })}</p>
        </div>
      </div>
      <p className="text-sm text-zinc-400">{t("about.description")}</p>
      <div className="flex flex-wrap gap-3">
        <button onClick={() => openExternal(REPOSITORY_URL)} className={`${link} bg-zinc-700 text-zinc-200 hover:bg-zinc-600`}>
          <GitHubIcon size={15} />
          {t("about.github")}
        </button>
        <button onClick={() => openExternal(WEBSITE_URL)} className={`${link} bg-zinc-700 text-zinc-200 hover:bg-zinc-600`}>
          <GlobeIcon size={15} />
          {t("about.website")}
        </button>
        {/* Amarillo de Buy Me a Coffee, como en la web y en los demás proyectos */}
        <button onClick={() => openExternal(DONATION_URL)} className={`${link} bg-[#ffdd00] text-zinc-950 font-semibold hover:bg-[#f5d000]`}>
          <CoffeeIcon size={15} />
          {t("about.donate")}
        </button>
      </div>
      <p className="text-xs text-zinc-500">{t("about.donateNote")}</p>

      {/* Actualizaciones */}
      <section className="pt-6 border-t border-zinc-800 space-y-3">
        <h3 className="text-sm font-medium text-zinc-300">{t("about.updates")}</h3>
        <label className="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            checked={checkUpdates}
            onChange={(e) => onCheckUpdatesChange(e.target.checked)}
            className="rounded border-zinc-600 bg-zinc-700 text-indigo-600"
          />
          {t("about.checkAutomatically")}
        </label>
        <div className="flex flex-wrap items-center gap-3">
          <button
            onClick={() => checkNow(true)}
            disabled={updatePhase === "checking" || updatePhase === "downloading" || updatePhase === "installing"}
            className={`${link} bg-zinc-700 text-zinc-200 hover:bg-zinc-600 disabled:opacity-50`}
          >
            {updatePhase === "checking" ? t("about.checking") : t("about.checkNow")}
          </button>
          {update && (
            <button onClick={openUpdate} className={`${link} bg-indigo-600 text-white hover:bg-indigo-500`}>
              {t("update.banner", { version: update.version })}
            </button>
          )}
          {!update && updatePhase === "upToDate" && <span className="text-sm text-green-400">{t("about.upToDate")}</span>}
          {updatePhase === "error" && !update && (
            <span className="text-sm text-red-400 break-words" title={updateError ?? ""}>
              {t("about.checkFailed")}
            </span>
          )}
        </div>
      </section>
    </div>
  );
}
