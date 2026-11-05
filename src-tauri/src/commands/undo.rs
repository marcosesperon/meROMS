//! «Deshacer»: la última operación que movió o renombró ficheros (Renombrar, Mover a…,
//! Normalizar media, Intercambiar tipos). Solo se recuerda la última, en memoria (se olvida
//! al cerrar la app). Lo que va a la papelera no: eso se recupera desde la papelera del sistema.
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
#[derive(Debug, Clone)]
pub enum RefUndo {
    None,
    /// Renombrar: colecciones y .m3u de un sistema (pares antigua → nueva)
    Rename { system: String, pairs: Vec<(String, String)> },
    /// Mover a…: colecciones entre dos sistemas
    Move { from: String, to: String, pairs: Vec<(String, String)> },
}

#[derive(Debug, Clone)]
pub struct UndoOp {
    /// rename | move | normalize | swap
    pub kind: String,
    pub system: String,
    pub files: Vec<Journaled>,
    pub games: Vec<GameChange>,
    pub refs: RefUndo,
}

static LAST: Mutex<Option<UndoOp>> = Mutex::new(None);

/// Guarda la operación como «la última» (si movió algo). El diario ya se ha recogido.
pub fn record(op: UndoOp) {
    if op.files.is_empty() && op.games.is_empty() {
        return;
    }
    if let Ok(mut last) = LAST.lock() {
        *last = Some(op);
    }
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
#[tauri::command]
pub fn undo_status() -> Option<UndoInfo> {
    let last = LAST.lock().ok()?;
    let op = last.as_ref()?;
    let name = Path::new(&op.system).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Some(UndoInfo { kind: op.kind.clone(), system: name, files: op.files.len() })
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

fn undo_op(layout: &Layout, op: &UndoOp) -> Result<UndoResult, String> {
    let mut result = UndoResult::default();
    undo_files(&op.files, &mut result);

    // Gamelists de los sistemas tocados
    let mut systems: Vec<String> = op.games.iter().flat_map(|c| [c.before_sys.clone(), c.after_sys.clone()]).collect();
    systems.sort();
    systems.dedup();
    let mut lists: HashMap<String, GameList> = HashMap::new();
    for s in &systems {
        lists.insert(s.clone(), super::gamelist::read_gamelist(layout, &SystemRef::from_rom_dir(s))?);
    }
    undo_gamelists(&mut lists, &op.games);
    if layout.writes_media_tags() || op.games.iter().any(|c| c.before.path != c.after.path || c.before_sys != c.after_sys) {
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
#[tauri::command]
pub fn undo_last(app: tauri::AppHandle) -> Result<UndoResult, String> {
    let op = LAST.lock().map_err(|e| e.to_string())?.take().ok_or("Nothing to undo")?;
    // Por si una operación anterior falló a medias y dejó el diario abierto: deshacer no se apunta
    let _ = media::journal_take();
    undo_op(&super::active_layout(&app)?, &op)
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
            games: vec![Game { path: "./Zelda (USA).sfc".into(), name: Some("Zelda".into()), image: Some("./covers/Zelda (USA).png".into()), ..Default::default() }],
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
}
