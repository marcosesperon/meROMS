import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "../../i18n";
import { detectEsdePaths, planMigration, runMigration } from "../../commands";
import type { MigrationMode, MigrationProgress, MigrationReport, PendingStructure, Profile } from "../../types";
import { useAppStore } from "../../store/useAppStore";
import { createProfile, getActiveProfile } from "../../utils/profiles";
import { askConfirm } from "../../utils/dialogs";

interface MigrationModalProps {
  open: boolean;
  onClose: () => void;
  /**
   * Modo «rellenado» (sistemas detectados en la otra estructura): el origen es el detectado,
   * el destino es el perfil activo y solo se migran esos sistemas. No se elige destino.
   */
  preset?: PendingStructure | null;
  /** Modo normal: perfil a migrar (desde Ajustes → Perfiles). Sin él, el activo. */
  sourceProfile?: Profile | null;
}

type Step = "config" | "preview" | "running" | "done";

/**
 * Migrador de estructura de un perfil (clásica ↔ ES-DE), en cuatro pasos:
 * configurar destino → vista previa → ejecución con progreso → informe.
 * Al acabar ofrece crear (o activar) el perfil de destino.
 */
export function MigrationModal({ open, onClose, preset, sourceProfile }: MigrationModalProps) {
  const { t } = useTranslation();
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const reloadLibrary = useAppStore((s) => s.reloadLibrary);
  const activeProfile = getActiveProfile(settings);
  // Normal: del perfil elegido (o el activo) a otro. Rellenado: de lo detectado al perfil activo.
  const source = preset ? preset.source : sourceProfile ?? activeProfile;
  const targetKind = source?.kind === "esde" ? "classic" : "esde";
  const kindLabel = (kind: Profile["kind"]) => (kind === "esde" ? "ES-DE" : t("profiles.kindClassic"));

  // Perfiles ya guardados que podrían ser el destino: la otra estructura y la misma carpeta de ROMs
  const candidates = useMemo(
    () => (settings.profiles ?? []).filter((p) => p.kind === targetKind && p.rom_root_path === source?.rom_root_path),
    [settings.profiles, targetKind, source?.rom_root_path]
  );

  const [step, setStep] = useState<Step>("config");
  const [target, setTarget] = useState<Profile | null>(null);
  const [mode, setMode] = useState<MigrationMode>("copy");
  const [report, setReport] = useState<MigrationReport | null>(null);
  const [progress, setProgress] = useState<MigrationProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Al abrir: destino = el primer perfil compatible o uno nuevo con nombre sugerido
  useEffect(() => {
    if (!open || !source) return;
    setStep("config");
    setMode("copy");
    setReport(null);
    setProgress(null);
    setError(null);
    setTarget(preset ? activeProfile ?? null : candidates[0] ?? newTarget());
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const unlisten = listen<MigrationProgress>("migration-progress", (e) => setProgress(e.payload));
    return () => { unlisten.then((fn) => fn()); };
  }, [open]);

  function newTarget(): Profile {
    const suffix = targetKind === "esde" ? "ES-DE" : t("profiles.kindClassic");
    return { ...createProfile(`${source?.name ?? ""} (${suffix})`, targetKind), rom_root_path: source?.rom_root_path ?? "" };
  }

  const isNewTarget = !!target && !(settings.profiles ?? []).some((p) => p.id === target.id);
  const targetReady = !!target && (target.kind !== "esde" || !!target.esde_home_path);

  async function handlePickEsdeHome() {
    if (!target) return;
    const selected = await openDialog({ directory: true, multiple: false });
    if (!selected) return;
    const folder = String(selected);
    try {
      const paths = await detectEsdePaths(folder);
      const defaultMedia = `${folder}/downloaded_media`;
      setTarget({ ...target, esde_home_path: folder, media_dir: paths.media_dir === defaultMedia ? undefined : paths.media_dir });
    } catch {
      setTarget({ ...target, esde_home_path: folder });
    }
  }

  async function handlePreview() {
    if (!source || !target) return;
    setBusy(true);
    setError(null);
    try {
      setReport(await planMigration(source, target, mode, preset?.systems));
      setStep("preview");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleRun() {
    if (!source || !target) return;
    if (mode === "move" && !(await askConfirm(t("migration.moveConfirm")))) return;
    setStep("running");
    setProgress(null);
    setError(null);
    try {
      setReport(await runMigration(source, target, mode, preset?.systems));
      setStep("done");
    } catch (e) {
      setError(String(e));
      setStep("preview");
    }
  }

  // Rellenado: el destino ya es el perfil activo; solo hay que volver a escanear
  async function handleFinishPreset() {
    onClose();
    await reloadLibrary();
  }

  // Crea el perfil de destino si es nuevo y lo deja activo
  async function handleActivateTarget() {
    if (!target) return;
    const profiles = settings.profiles ?? [];
    const updatedProfiles = isNewTarget ? [...profiles, target] : profiles;
    await updateSettings({ ...settings, profiles: updatedProfiles, active_profile_id: target.id });
    onClose();
  }

  if (!open || !source) return null;

  const totals = report?.systems.reduce(
    (acc, s) => ({
      media: acc.media + s.media_transferred,
      skipped: acc.skipped + s.media_skipped_count,
      unsupported: acc.unsupported + s.media_unsupported,
      renamed: acc.renamed + s.media_renamed,
      gamelists: acc.gamelists + (s.has_gamelist ? 1 : 0),
    }),
    { media: 0, skipped: 0, unsupported: 0, renamed: 0, gamelists: 0 }
  );

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60]">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg w-[860px] max-h-[85vh] flex flex-col mx-4">
        <div className="px-5 py-4 border-b border-zinc-700">
          <h3 className="text-base font-semibold text-zinc-100">{t("migration.title")}</h3>
          <p className="text-sm text-zinc-400 mt-1">
            {t("migration.fromTo", {
              // Un origen detectado puede no tener nombre (la estructura clásica de esta carpeta)
              from: source.name ? `${source.name} (${kindLabel(source.kind)})` : kindLabel(source.kind),
              to: preset && target ? `${target.name} (${kindLabel(target.kind)})` : kindLabel(targetKind),
            })}
          </p>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
          {error && <p className="text-sm text-red-400">{error}</p>}

          {step === "config" && target && (
            <>
              <p className="text-xs text-zinc-500">{t("migration.description")}</p>

              {preset && (
                <p className="text-sm text-zinc-300">{t("structure.presetSystems", { systems: preset.systems.join(", ") })}</p>
              )}

              {/* Destino: perfil existente o nuevo (en modo rellenado ya está fijado) */}
              {!preset && (
              <section className="space-y-2">
                <label className="text-xs text-zinc-400 block">{t("migration.target")}</label>
                <select
                  value={isNewTarget ? "__new__" : target.id}
                  onChange={(e) =>
                    setTarget(e.target.value === "__new__" ? newTarget() : candidates.find((c) => c.id === e.target.value) ?? null)
                  }
                  className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200"
                >
                  {candidates.map((c) => (
                    <option key={c.id} value={c.id}>{c.name}</option>
                  ))}
                  <option value="__new__">{t("migration.newProfile")}</option>
                </select>

                {isNewTarget && (
                  <input
                    type="text"
                    value={target.name}
                    onChange={(e) => setTarget({ ...target, name: e.target.value })}
                    placeholder={t("profiles.name")}
                    className="w-full bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200"
                  />
                )}

                {target.kind === "esde" && (
                  <div className="flex gap-3">
                    <input
                      type="text"
                      readOnly
                      value={target.esde_home_path ?? ""}
                      placeholder={t("profiles.esdeHome")}
                      className="flex-1 bg-zinc-900 border border-zinc-600 rounded px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500"
                    />
                    <button
                      onClick={handlePickEsdeHome}
                      disabled={!isNewTarget}
                      className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50"
                    >
                      {t("settings.browse")}
                    </button>
                  </div>
                )}
                <p className="text-xs text-zinc-500">{t("migration.romsStay", { path: source.rom_root_path })}</p>
              </section>
              )}

              {/* Copiar o mover */}
              <section className="space-y-2">
                <label className="text-xs text-zinc-400 block">{t("migration.mode")}</label>
                {(["copy", "move"] as MigrationMode[]).map((m) => (
                  <label key={m} className="flex items-start gap-2 text-sm text-zinc-300 cursor-pointer">
                    <input type="radio" checked={mode === m} onChange={() => setMode(m)} className="mt-1" />
                    <span>
                      {t(m === "copy" ? "migration.modeCopy" : "migration.modeMove")}
                      <span className="block text-xs text-zinc-500">
                        {t(m === "copy" ? "migration.modeCopyHelp" : "migration.modeMoveHelp")}
                      </span>
                    </span>
                  </label>
                ))}
              </section>
            </>
          )}

          {step === "running" && (
            <div className="py-8 space-y-3">
              <p className="text-sm text-zinc-300 text-center">
                {t("migration.running")} {progress?.system ?? ""}
              </p>
              <div className="w-full bg-zinc-900 rounded-full h-2 overflow-hidden">
                <div
                  className="bg-indigo-500 h-full transition-all duration-200"
                  style={{ width: progress ? `${(progress.current / progress.total) * 100}%` : "0%" }}
                />
              </div>
              {progress && <p className="text-xs text-zinc-500 text-center">{progress.current} / {progress.total}</p>}
            </div>
          )}

          {(step === "preview" || step === "done") && report && totals && (
            <>
              <p className={`text-sm ${step === "done" ? "text-emerald-300" : "text-zinc-300"}`}>
                {t(step === "done" ? "migration.summaryDone" : "migration.summaryPlan", {
                  gamelists: totals.gamelists,
                  media: totals.media,
                  collections: report.collections.length,
                })}
              </p>

              {/* Avisos importantes primero */}
              {/* Informativo: los medios con nombre antiguo pasan ya con el nombre de la ROM */}
              {totals.renamed > 0 && (
                <p className="text-xs text-zinc-400">{t("migration.renamed", { count: totals.renamed })}</p>
              )}
              {totals.unsupported > 0 && (
                <p className="text-xs text-amber-400">{t("migration.unsupported", { count: totals.unsupported })}</p>
              )}
              {totals.skipped > 0 && (
                <p className="text-xs text-amber-400">{t("migration.skipped", { count: totals.skipped })}</p>
              )}
              {report.warnings.map((w) => (
                <p key={w} className="text-xs text-amber-400">{w}</p>
              ))}

              <div className="border border-zinc-700 rounded overflow-hidden">
                <table className="w-full text-sm">
                  <thead className="bg-zinc-900 text-xs text-zinc-500">
                    <tr>
                      <th className="text-left px-3 py-2">{t("migration.colSystem")}</th>
                      <th className="text-right px-3 py-2">{t("migration.colGames")}</th>
                      <th className="text-right px-3 py-2">{t("migration.colMedia")}</th>
                      <th className="text-right px-3 py-2">{t("migration.colSkipped")}</th>
                      <th className="text-left px-3 py-2">{t("migration.colNotes")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-zinc-700/40">
                    {report.systems.map((s) => (
                      <tr key={s.name}>
                        <td className="px-3 py-1.5 text-zinc-200">{s.name}</td>
                        <td className="px-3 py-1.5 text-right text-zinc-400">{s.has_gamelist ? s.games : "—"}</td>
                        <td className="px-3 py-1.5 text-right text-zinc-400">{s.media_transferred}</td>
                        <td className="px-3 py-1.5 text-right text-zinc-400" title={s.media_skipped.join("\n")}>
                          {s.media_skipped_count || ""}
                        </td>
                        <td className="px-3 py-1.5 text-xs text-zinc-500">
                          {s.dest_gamelist_exists && t("migration.destGamelistExists")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>

        <div className="border-t border-zinc-700 px-5 py-3 flex justify-between gap-2">
          <button
            onClick={step === "preview" ? () => setStep("config") : step === "done" && preset ? handleFinishPreset : onClose}
            disabled={step === "running"}
            className="px-4 py-2 text-sm bg-zinc-700 text-zinc-300 rounded hover:bg-zinc-600 transition-colors disabled:opacity-50"
          >
            {step === "preview" ? t("migration.back") : t("rename.close")}
          </button>
          {step === "config" && (
            <button
              onClick={handlePreview}
              disabled={!targetReady || busy}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors disabled:opacity-50"
            >
              {busy ? "..." : t("migration.preview")}
            </button>
          )}
          {step === "preview" && (
            <button
              onClick={handleRun}
              className="px-4 py-2 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-500 transition-colors"
            >
              {t(mode === "copy" ? "migration.runCopy" : "migration.runMove")}
            </button>
          )}
          {step === "done" && preset && (
            <button
              onClick={handleFinishPreset}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
            >
              {t("structure.finish")}
            </button>
          )}
          {step === "done" && !preset && (
            <button
              onClick={handleActivateTarget}
              className="px-4 py-2 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
            >
              {t(isNewTarget ? "migration.createAndActivate" : "migration.activate")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
