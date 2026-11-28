//! «Deshacer»: la última operación que movió o renombró ficheros (Renombrar, Mover a…,
//! Normalizar media, Intercambiar tipos, Apartar). Solo se recuerda la última, en memoria (se
//! olvida al cerrar la app). Lo que va a la papelera no: eso se recupera desde la papelera del
//! sistema. «Ponerlo todo en orden» junta sus pasos en una sola operación (`group_begin` /
//! `group_end`), que se deshace entera, del último paso al primero.
//!
//! Se guardan los pasos de ficheros tal cual se dieron (el diario de `media.rs`), una foto de
//! antes y después de cada juego tocado (ruta y campos de media) y qué hay que revertir en
//! colecciones y .m3u. Al deshacer, lo editado después en la gamelist se respeta: un campo
//! solo vuelve a su valor si sigue con el que dejó la operación.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use crate::layout::{Layout, SystemRef};
use crate::media::{self, Journaled};
use crate::models::{Game, GameList};

/// Un juego tocado por la operación: dónde y cómo estaba, y dónde y cómo quedó
#[derive(Debug, Clone)]
pub struct GameChange {
    pub before_sys: String,
    pub before: Game,
    pub after_sys: String,
    pub after: Game,
}

/// Qué hay que revertir fuera de la gamelist
#[derive(Debug, Clone, Default)]
pub enum RefUndo {
    #[default]
    None,
    /// Renombrar: colecciones y .m3u de un sistema (pares antigua → nueva)
    Rename { system: String, pairs: Vec<(String, String)> },
    /// Mover a…: colecciones entre dos sistemas
    Move { from: String, to: String, pairs: Vec<(String, String)> },
}

#[derive(Debug, Clone, Default)]
pub struct UndoOp {
    /// rename | move | normalize | swap | set_aside | order (varias juntas)
    pub kind: String,
    pub system: String,
    pub files: Vec<Journaled>,
    pub games: Vec<GameChange>,
    pub refs: RefUndo,
    /// Entradas que la operación sacó de la gamelist (apartar), con su sistema: vuelven al deshacer
    pub removed: Vec<(String, Game)>,
    /// Entradas que la operación añadió (sistema, ruta): al deshacer se quitan (importar)
    pub added: Vec<(String, String)>,
    /// Entradas repetidas fusionadas (sistema, la que quedó, las de antes): vuelven al deshacer
    pub merged: Vec<(String, Game, Vec<Game>)>,
    /// Operación compuesta (Ponerlo todo en orden): sus partes, en el orden en que se hicieron
    pub parts: Vec<UndoOp>,
}

impl UndoOp {
    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.games.is_empty() && self.removed.is_empty() && self.added.is_empty() && self.merged.is_empty() && self.parts.is_empty()
    }

    /// Ficheros que mueve, contando los de sus partes
    fn file_count(&self) -> usize {
        self.files.len() + self.parts.iter().map(UndoOp::file_count).sum::<usize>()
    }
}

static LAST: Mutex<Option<UndoOp>> = Mutex::new(None);
/// Mientras hay un grupo abierto, las operaciones se juntan aquí en lugar de ser «la última»
static GROUP: Mutex<Option<Vec<UndoOp>>> = Mutex::new(None);

/// Guarda la operación como «la última» (si movió algo), o como una parte más del grupo abierto.
/// El diario ya se ha recogido.
pub fn record(op: UndoOp) {
    if op.is_empty() {
        return;
    }
    if let Ok(mut group) = GROUP.lock() {
        if let Some(parts) = group.as_mut() {
            parts.push(op);
            return;
        }
    }
    if let Ok(mut last) = LAST.lock() {
        *last = Some(op);
    }
}

/// Empieza a juntar operaciones (Ponerlo todo en orden). Un grupo que quedara abierto se descarta.
pub fn group_begin() {
    if let Ok(mut group) = GROUP.lock() {
        *group = Some(Vec::new());
    }
}

/// Cierra el grupo: si algo se movió, queda como «la última» operación (`kind`), una sola
pub fn group_end(kind: &str, system: &str) {
    let parts = GROUP.lock().ok().and_then(|mut g| g.take()).unwrap_or_default();
    if parts.is_empty() {
        return;
    }
    let op = if parts.len() == 1 {
        parts.into_iter().next().unwrap_or_default()
    } else {
        UndoOp { kind: kind.to_string(), system: system.to_string(), parts, ..Default::default() }
    };
    if let Ok(mut last) = LAST.lock() {
        *last = Some(op);
    }
}

/// Abre un grupo de deshacer (lo usa «Ponerlo todo en orden» antes del primer paso)
#[tauri::command(async)]
pub fn undo_group_begin() {
    group_begin();
}

/// Cierra el grupo: todos sus pasos se deshacen de una vez
#[tauri::command(async)]
pub fn undo_group_end(system_path: String) {
    group_end("order", &system_path);
}

/// Juegos cuya ruta cambió (`pairs` antigua → nueva), con su foto de antes y de después
pub fn changes_by_path(before_sys: &str, before: &GameList, after_sys: &str, after: &GameList, pairs: &[(String, String)]) -> Vec<GameChange> {
    let old: HashMap<&str, &Game> = before.games.iter().map(|g| (g.path.as_str(), g)).collect();
    let new: HashMap<&str, &Game> = after.games.iter().map(|g| (g.path.as_str(), g)).collect();
    pairs
        .iter()
        .filter_map(|(o, n)| {
            Some(GameChange {
                before_sys: before_sys.to_string(),
                before: (*old.get(o.as_str())?).clone(),
                after_sys: after_sys.to_string(),
                after: (*new.get(n.as_str())?).clone(),
            })
        })
        .collect()
}

/// Juegos con la misma ruta cuyos campos de media cambiaron (Normalizar, Intercambiar)
pub fn changes_same_paths(sys: &str, before: &GameList, after: &GameList) -> Vec<GameChange> {
    let old: HashMap<&str, &Game> = before.games.iter().map(|g| (g.path.as_str(), g)).collect();
    after
        .games
        .iter()
        .filter_map(|a| {
            let b = old.get(a.path.as_str())?;
            let changed = media::media_fields().any(|f| media::get_field(b, f) != media::get_field(a, f));
            changed.then(|| GameChange { before_sys: sys.to_string(), before: (*b).clone(), after_sys: sys.to_string(), after: a.clone() })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct UndoInfo {
    pub kind: String,
    pub system: String,
    pub files: usize,
}

/// Qué se puede deshacer ahora (None = nada)
#[tauri::command(async)]
pub fn undo_status() -> Option<UndoInfo> {
    let last = LAST.lock().ok()?;
    let op = last.as_ref()?;
    let name = Path::new(&op.system).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Some(UndoInfo { kind: op.kind.clone(), system: name, files: op.file_count() })
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UndoResult {
    pub restored: usize,
    /// Pasos que no se pudieron deshacer, con el motivo
    pub skipped: Vec<String>,
}

/// Devuelve los ficheros a su sitio, en orden inverso. Nunca sobrescribe.
fn undo_files(files: &[Journaled], result: &mut UndoResult) {
    for step in files.iter().rev() {
        match step {
            Journaled::Moved(from, to) => {
                if !to.exists() {
                    result.skipped.push(format!("{} (no longer there)", to.display()));
                } else if from.exists() {
                    result.skipped.push(format!("{} (its original place is taken)", from.display()));
                } else {
                    match media::move_file(to, from) {
                        Ok(()) => result.restored += 1,
                        Err(e) => result.skipped.push(format!("{} ({})", to.display(), e)),
                    }
                }
            }
            Journaled::Copied(dest) => {
                if dest.is_file() && std::fs::remove_file(dest).is_ok() {
                    result.restored += 1;
                }
            }
            Journaled::Rewritten(path, before) => {
                if path.is_file() && std::fs::write(path, before).is_ok() {
                    result.restored += 1;
                } else {
                    result.skipped.push(format!("{} (no longer there)", path.display()));
                }
            }
        }
    }
}

/// Devuelve cada juego a su ruta (y a su gamelist si cambió de sistema). Un campo de media
/// solo vuelve atrás si sigue con el valor que dejó la operación: lo editado después se queda.
fn undo_gamelists(lists: &mut HashMap<String, GameList>, games: &[GameChange]) {
    for change in games {
        let Some(list) = lists.get_mut(&change.after_sys) else { continue };
        let Some(pos) = list.games.iter().position(|g| g.path == change.after.path) else { continue };
        let mut game = list.games[pos].clone();
        game.path = change.before.path.clone();
        // El título (renombrar le quita el número de recopilatorio), si nadie lo cambió después
        if game.name == change.after.name {
            game.name = change.before.name.clone();
        }
        // El emulador del juego (Emulador por juego), igual: si nadie lo cambió después
        if game.altemulator == change.after.altemulator {
            game.altemulator = change.before.altemulator.clone();
        }
        for field in media::media_fields() {
            if media::get_field(&game, field) == media::get_field(&change.after, field) {
                media::set_field(&mut game, field, media::get_field(&change.before, field).cloned());
            }
        }
        if change.before_sys == change.after_sys {
            list.games[pos] = game;
        } else {
            list.games.remove(pos);
            if let Some(origin) = lists.get_mut(&change.before_sys) {
                origin.games.push(game);
            }
        }
    }
}

/// Vuelve a meter en su gamelist las entradas que se apartaron (si nadie ha puesto otra con esa ruta)
fn restore_removed(lists: &mut HashMap<String, GameList>, removed: &[(String, Game)]) {
    for (sys, game) in removed {
        let Some(list) = lists.get_mut(sys) else { continue };
        if !list.games.iter().any(|g| g.path == game.path) {
            list.games.push(game.clone());
        }
    }
}

/// Vuelve a poner las entradas repetidas que se fusionaron, en el sitio de la fusionada, si
/// sigue ahí (una sola entrada con esa ruta)
fn unmerge(lists: &mut HashMap<String, GameList>, merged: &[(String, Game, Vec<Game>)]) {
    for (sys, after, before) in merged {
        let Some(list) = lists.get_mut(sys) else { continue };
        let positions: Vec<usize> = list.games.iter().enumerate().filter(|(_, g)| g.path == after.path).map(|(i, _)| i).collect();
        if let [pos] = positions[..] {
            list.games.splice(pos..=pos, before.iter().cloned());
        }
    }
}

fn undo_op(layout: &Layout, op: &UndoOp) -> Result<UndoResult, String> {
    let mut result = UndoResult::default();
    // Compuesta: cada parte al revés, de la última a la primera
    if !op.parts.is_empty() {
        for part in op.parts.iter().rev() {
            let r = undo_op(layout, part)?;
            result.restored += r.restored;
            result.skipped.extend(r.skipped);
        }
        return Ok(result);
    }
    undo_files(&op.files, &mut result);

    // Gamelists de los sistemas tocados
    let mut systems: Vec<String> = op.games.iter().flat_map(|c| [c.before_sys.clone(), c.after_sys.clone()]).collect();
    systems.extend(op.removed.iter().map(|(sys, _)| sys.clone()));
    systems.extend(op.merged.iter().map(|(sys, _, _)| sys.clone()));
    systems.extend(op.added.iter().map(|(sys, _)| sys.clone()));
    systems.sort();
    systems.dedup();
    let mut lists: HashMap<String, GameList> = HashMap::new();
    for s in &systems {
        lists.insert(s.clone(), super::gamelist::read_gamelist(layout, &SystemRef::from_rom_dir(s))?);
    }
    undo_gamelists(&mut lists, &op.games);
    restore_removed(&mut lists, &op.removed);
    unmerge(&mut lists, &op.merged);
    for (sys, path) in &op.added {
        if let Some(list) = lists.get_mut(sys) {
            list.games.retain(|g| &g.path != path);
        }
    }
    // Hay que escribir la gamelist si cambia algo de ella: rutas, entradas, títulos, emuladores
    // (con la estructura clásica, también las rutas de los medios)
    let entries_changed = op.games.iter().any(|c| {
        c.before.path != c.after.path || c.before_sys != c.after_sys || c.before.name != c.after.name || c.before.altemulator != c.after.altemulator
    });
    if layout.writes_media_tags() || !op.removed.is_empty() || !op.merged.is_empty() || !op.added.is_empty() || entries_changed {
        for (s, list) in &lists {
            super::gamelist::write_gamelist(layout, &SystemRef::from_rom_dir(s), list)?;
        }
    }

    // Colecciones y .m3u, con los pares al revés
    match &op.refs {
        RefUndo::None => {}
        RefUndo::Rename { system, pairs } => {
            let back: Vec<(String, String)> = pairs.iter().map(|(o, n)| (n.clone(), o.clone())).collect();
            super::rename::update_references(layout, &SystemRef::from_rom_dir(system), &back);
        }
        RefUndo::Move { from, to, pairs } => {
            let back: Vec<(String, String)> = pairs.iter().map(|(o, n)| (n.clone(), o.clone())).collect();
            super::move_games::update_collections(layout, &SystemRef::from_rom_dir(to), &SystemRef::from_rom_dir(from), &back);
        }
    }
    Ok(result)
}

/// Deshace la última operación y la olvida
#[tauri::command(async)]
pub fn undo_last(app: tauri::AppHandle) -> Result<UndoResult, String> {
    let op = LAST.lock().map_err(|e| e.to_string())?.take().ok_or("Nothing to undo")?;
    // Por si una operación anterior falló a medias y dejó el diario abierto: deshacer no se apunta
    let _ = media::journal_take();
    if let Ok(mut group) = GROUP.lock() {
        *group = None;
    }
    undo_op(&super::active_layout(&app)?, &op)
}

/// Para los tests de otros módulos: deshacer una operación sin pasar por «la última» (global)
#[cfg(test)]
pub fn undo_op_for_tests(layout: &Layout, op: &UndoOp) -> Result<UndoResult, String> {
    undo_op(layout, op)
}

/// Para los tests: la ruta absoluta de un juego
#[cfg(test)]
fn rom(system: &SystemRef, path: &str) -> std::path::PathBuf {
    system.rom_dir.join(crate::layout::strip_dot_slash(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn deshacer_una_fusion_devuelve_las_entradas_repetidas_a_su_sitio() {
        let g = |path: &str, name: &str| Game { path: path.into(), name: Some(name.into()), ..Default::default() };
        let before = vec![g("./Rep.bin", "Uno"), g("./Rep.bin", "Dos")];
        let merged = g("./Rep.bin", "Dos");
        let mut lists = HashMap::new();
        lists.insert("sys".to_string(), GameList { games: vec![g("./A.bin", "A"), merged.clone(), g("./B.bin", "B")], ..Default::default() });
        unmerge(&mut lists, &[("sys".to_string(), merged, before)]);
        let names: Vec<_> = lists["sys"].games.iter().map(|x| x.name.clone().unwrap()).collect();
        assert_eq!(names, ["A", "Uno", "Dos", "B"]);
    }

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn a_rename_is_undone_keeping_what_was_edited_afterwards() {
        let tmp = tempfile::tempdir().unwrap();
        let sys = SystemRef::from_rom_dir(tmp.path().join("snes"));
        let layout = Layout::Classic;
        touch(&rom(&sys, "./Zelda (USA).sfc"), "rom");
        touch(&sys.rom_dir.join("covers/Zelda (USA).png"), "img");
        touch(&sys.rom_dir.join("custom-Favs.cfg"), "./Zelda (USA).sfc\n");
        let before = GameList {
            games: vec![Game { path: "./Zelda (USA).sfc".into(), name: Some("0001 Zelda".into()), image: Some("./covers/Zelda (USA).png".into()), ..Default::default() }],
            ..Default::default()
        };
        super::super::gamelist::write_gamelist_no_backup(&layout, &sys, &before).unwrap();

        // La operación: renombrar a «Zelda.sfc» (como hace rename_roms)
        media::journal_begin();
        media::move_file(&rom(&sys, "./Zelda (USA).sfc"), &rom(&sys, "./Zelda.sfc")).unwrap();
        media::move_file(&sys.rom_dir.join("covers/Zelda (USA).png"), &sys.rom_dir.join("covers/Zelda.png")).unwrap();
        let mut after = before.clone();
        after.games[0].path = "./Zelda.sfc".into();
        after.games[0].image = Some("./covers/Zelda.png".into());
        // Renombrar también pudo quitarle al título el número de recopilatorio
        after.games[0].name = Some("Zelda".into());
        super::super::gamelist::write_gamelist_no_backup(&layout, &sys, &after).unwrap();
        super::super::rename::update_references(&layout, &sys, &[("./Zelda (USA).sfc".into(), "./Zelda.sfc".into())]);
        let pairs = vec![("./Zelda (USA).sfc".to_string(), "./Zelda.sfc".to_string())];
        let sys_s = sys.rom_dir.to_string_lossy().to_string();
        let op = UndoOp {
            kind: "rename".into(),
            system: sys_s.clone(),
            files: media::journal_take(),
            games: changes_by_path(&sys_s, &before, &sys_s, &after, &pairs),
            refs: RefUndo::Rename { system: sys_s.clone(), pairs },
            ..Default::default()
        };

        // Después se edita la descripción: deshacer no la toca
        let mut edited = after.clone();
        edited.games[0].desc = Some("Editado después".into());
        super::super::gamelist::write_gamelist_no_backup(&layout, &sys, &edited).unwrap();

        let r = undo_op(&layout, &op).unwrap();
        assert_eq!(r.restored, 2, "{:?}", r.skipped);
        assert!(rom(&sys, "./Zelda (USA).sfc").is_file());
        assert!(sys.rom_dir.join("covers/Zelda (USA).png").is_file());
        let list = super::super::gamelist::read_gamelist(&layout, &sys).unwrap();
        assert_eq!(list.games[0].path, "./Zelda (USA).sfc");
        assert_eq!(list.games[0].image.as_deref(), Some("./covers/Zelda (USA).png"));
        assert_eq!(list.games[0].desc.as_deref(), Some("Editado después"));
        assert_eq!(list.games[0].name.as_deref(), Some("0001 Zelda"), "the title comes back too");
        assert_eq!(fs::read_to_string(sys.rom_dir.join("custom-Favs.cfg")).unwrap(), "./Zelda (USA).sfc\n");
    }

    #[test]
    fn a_move_between_systems_goes_back_and_a_taken_place_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::Classic;
        let from = SystemRef::from_rom_dir(tmp.path().join("gbc"));
        let to = SystemRef::from_rom_dir(tmp.path().join("gb"));
        touch(&rom(&from, "./Tetris.gb"), "rom");
        touch(&rom(&from, "./Otro.gbc"), "x");
        let before = GameList { games: vec![Game { path: "./Tetris.gb".into(), ..Default::default() }, Game { path: "./Otro.gbc".into(), ..Default::default() }], ..Default::default() };
        super::super::gamelist::write_gamelist_no_backup(&layout, &from, &before).unwrap();

        media::journal_begin();
        media::move_file(&rom(&from, "./Tetris.gb"), &rom(&to, "./Tetris.gb")).unwrap();
        media::move_file(&rom(&from, "./Otro.gbc"), &rom(&to, "./Otro.gbc")).unwrap();
        let src_after = GameList::default();
        let dst_after = before.clone();
        super::super::gamelist::write_gamelist_no_backup(&layout, &from, &src_after).unwrap();
        super::super::gamelist::write_gamelist_no_backup(&layout, &to, &dst_after).unwrap();
        let pairs = vec![("./Tetris.gb".to_string(), "./Tetris.gb".to_string()), ("./Otro.gbc".to_string(), "./Otro.gbc".to_string())];
        let (fs_, ts) = (from.rom_dir.to_string_lossy().to_string(), to.rom_dir.to_string_lossy().to_string());
        let op = UndoOp {
            kind: "move".into(),
            system: fs_.clone(),
            files: media::journal_take(),
            games: changes_by_path(&fs_, &before, &ts, &dst_after, &pairs),
            refs: RefUndo::Move { from: fs_.clone(), to: ts.clone(), pairs },
            ..Default::default()
        };
        // Alguien ocupa el sitio original de uno
        touch(&rom(&from, "./Otro.gbc"), "nuevo");

        let r = undo_op(&layout, &op).unwrap();
        assert_eq!(r.restored, 1);
        assert_eq!(r.skipped.len(), 1);
        assert!(rom(&from, "./Tetris.gb").is_file());
        assert_eq!(fs::read_to_string(rom(&from, "./Otro.gbc")).unwrap(), "nuevo", "never overwritten");
        let back = super::super::gamelist::read_gamelist(&layout, &from).unwrap();
        assert_eq!(back.games.len(), 2, "both entries are back in their gamelist");
    }

    #[test]
    fn steps_through_temporary_names_are_undone_in_reverse() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        touch(&d.join("covers/J.png"), "captura");
        touch(&d.join("screenshots/J.png"), "caratula");
        // Intercambio con nombre temporal, como swap_media
        media::journal_begin();
        media::move_file(&d.join("covers/J.png"), &d.join("covers/J.tmp")).unwrap();
        media::move_file(&d.join("screenshots/J.png"), &d.join("covers/J.png")).unwrap();
        media::move_file(&d.join("covers/J.tmp"), &d.join("screenshots/J.png")).unwrap();
        let files = media::journal_take();
        let mut r = UndoResult::default();
        undo_files(&files, &mut r);
        assert_eq!(r.restored, 3);
        assert_eq!(fs::read_to_string(d.join("covers/J.png")).unwrap(), "captura");
        assert_eq!(fs::read_to_string(d.join("screenshots/J.png")).unwrap(), "caratula");
    }

    #[test]
    fn a_compound_operation_is_undone_from_the_last_part_to_the_first() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        touch(&d.join("A.png"), "a");
        // Dos partes: A → B y luego B → C. Al revés: C → B y B → A
        media::journal_begin();
        media::move_file(&d.join("A.png"), &d.join("B.png")).unwrap();
        let first = UndoOp { kind: "normalize".into(), files: media::journal_take(), ..Default::default() };
        media::journal_begin();
        media::move_file(&d.join("B.png"), &d.join("C.png")).unwrap();
        let second = UndoOp { kind: "rename".into(), files: media::journal_take(), ..Default::default() };
        let op = UndoOp { kind: "order".into(), parts: vec![first, second], ..Default::default() };
        assert_eq!(op.file_count(), 2);
        let r = undo_op(&Layout::Classic, &op).unwrap();
        assert_eq!(r.restored, 2, "{:?}", r.skipped);
        assert!(d.join("A.png").is_file());
        assert!(!d.join("C.png").exists());
    }

    #[test]
    fn a_rewritten_cue_and_its_renamed_tracks_go_back() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        touch(&d.join("J (USA).cue"), "FILE \"J (USA).bin\" BINARY\n");
        touch(&d.join("J (USA).bin"), "pista");
        media::journal_begin();
        media::move_file(&d.join("J (USA).cue"), &d.join("J.cue")).unwrap();
        media::move_file(&d.join("J (USA).bin"), &d.join("J.bin")).unwrap();
        let before = fs::read(d.join("J.cue")).unwrap();
        media::journal_rewrite(&d.join("J.cue"), before);
        fs::write(d.join("J.cue"), "FILE \"J.bin\" BINARY\n").unwrap();
        let mut r = UndoResult::default();
        undo_files(&media::journal_take(), &mut r);
        assert_eq!(r.restored, 3, "{:?}", r.skipped);
        assert_eq!(fs::read_to_string(d.join("J (USA).cue")).unwrap(), "FILE \"J (USA).bin\" BINARY\n");
        assert!(d.join("J (USA).bin").is_file());
    }
}
