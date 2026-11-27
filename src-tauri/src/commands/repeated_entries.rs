//! Entradas repetidas: la misma ROM (misma `<path>`) dos o más veces en la gamelist. Pasa con
//! gamelists juntadas a mano o de herramientas que añaden sin mirar si ya estaba. Mientras siguen
//! así, todo lo que busca un juego por su ruta encuentra solo la primera, y la otra se queda como
//! estaba (Normalizar la proponía una y otra vez).
//!
//! Al fusionar se queda una sola entrada por ROM:
//! - gana la que tiene el MD5 de la ROM (el que guardó el scraper); si ninguna, la más completa;
//!   si empatan, la primera (la que usa ES-DE);
//! - las otras solo rellenan los campos vacíos de la ganadora y los medios que no se encuentran,
//!   y solo con ficheros que existen; sus ids y hashes no (pueden ser de otro juego).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

use crate::layout::{strip_dot_slash, SystemRef};
use crate::media;
use crate::models::Game;

/// Campos que nunca se copian de la entrada que pierde: dicen qué juego es, y puede ser otro
const IDENTITY_FIELDS: &[&str] = &["@id", "@source", "path", "md5", "hash", "crc32", "cheevosHash", "cheevosId"];

/// Lo que se enseña de cada entrada
#[derive(Debug, Clone, Serialize)]
pub struct EntrySummary {
    pub name: Option<String>,
    pub year: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    /// Su MD5 es el de la ROM
    pub matches_rom: bool,
    /// Campos rellenos
    pub filled: usize,
}

/// Una ROM con varias entradas y cuál se queda
#[derive(Debug, Clone, Serialize)]
pub struct RepeatedEntry {
    pub path: String,
    pub entries: Vec<EntrySummary>,
    /// Posición (dentro de `entries`) de la que se queda
    pub keep: usize,
    /// md5 = la del MD5 de la ROM · complete = la más completa · first = empate, la primera
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MergeResult {
    /// ROMs que han quedado con una sola entrada
    pub merged: usize,
    /// Entradas quitadas (las que se fusionaron en otra)
    pub removed: usize,
}

/// Las ROMs con la entrada repetida (con `game_paths`, solo esas)
#[tauri::command(async)]
pub fn find_repeated_entries(app: tauri::AppHandle, system_path: String, game_paths: Option<Vec<String>>) -> Result<Vec<RepeatedEntry>, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    Ok(plan(&system, &gamelist.games, game_paths.as_deref()))
}

/// Deja una sola entrada para cada una de esas ROMs (sin `game_paths`, todas)
#[tauri::command(async)]
pub fn merge_repeated_entries(app: tauri::AppHandle, system_path: String, game_paths: Option<Vec<String>>) -> Result<MergeResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let mut gamelist = super::gamelist::read_gamelist(&layout, &system)?;
    let repeated = plan(&system, &gamelist.games, game_paths.as_deref());
    if repeated.is_empty() {
        return Ok(MergeResult::default());
    }
    let (result, undo) = merge(&system, &mut gamelist.games, &repeated);
    // write_gamelist() hace la copia de seguridad antes de escribir
    super::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    let sys = system.rom_dir.to_string_lossy().to_string();
    super::undo::record(super::undo::UndoOp {
        kind: "merge".into(),
        system: sys.clone(),
        merged: undo.into_iter().map(|(after, before)| (sys.clone(), after, before)).collect(),
        ..Default::default()
    });
    Ok(result)
}

/// Agrupa por ruta y decide la ganadora de cada grupo. No toca nada.
pub(super) fn plan(system: &SystemRef, games: &[Game], only: Option<&[String]>) -> Vec<RepeatedEntry> {
    let mut order: Vec<&str> = Vec::new();
    let mut by_path: HashMap<&str, Vec<&Game>> = HashMap::new();
    for game in games {
        let list = by_path.entry(game.path.as_str()).or_default();
        if list.is_empty() {
            order.push(game.path.as_str());
        }
        list.push(game);
    }
    order
        .into_iter()
        .filter(|path| by_path[path].len() > 1)
        .filter(|path| only.is_none_or(|o| o.iter().any(|p| p == path)))
        .map(|path| decide(system, path, &by_path[path]))
        .collect()
}

fn decide(system: &SystemRef, path: &str, entries: &[&Game]) -> RepeatedEntry {
    // El MD5 de la ROM solo se calcula si alguna entrada tiene uno con el que comparar
    let has_hash = entries.iter().any(|g| stored_md5(g).is_some());
    let rom_md5 = if has_hash { file_md5(&system.rom_dir.join(strip_dot_slash(path))) } else { None };
    let summaries: Vec<EntrySummary> = entries
        .iter()
        .map(|g| EntrySummary {
            name: g.name.clone(),
            year: g.releasedate.as_ref().filter(|d| d.len() >= 4).map(|d| d[..4].to_string()),
            developer: g.developer.clone(),
            publisher: g.publisher.clone(),
            matches_rom: rom_md5.is_some() && stored_md5(g) == rom_md5,
            filled: filled_fields(g),
        })
        .collect();
    let (keep, reason) = match summaries.iter().position(|s| s.matches_rom) {
        Some(i) => (i, "md5"),
        None => {
            // La más completa; si hay empate, la primera
            let best = summaries.iter().map(|s| s.filled).max().unwrap_or(0);
            let i = summaries.iter().position(|s| s.filled == best).unwrap_or(0);
            let tie = summaries.iter().filter(|s| s.filled == best).count() > 1;
            (i, if tie { "first" } else { "complete" })
        }
    };
    RepeatedEntry { path: path.to_string(), entries: summaries, keep, reason: reason.to_string() }
}

/// Fusiona cada grupo en la posición de su primera entrada. Devuelve, para Deshacer, cada
/// entrada fusionada con las que había antes.
pub(super) fn merge(system: &SystemRef, games: &mut Vec<Game>, repeated: &[RepeatedEntry]) -> (MergeResult, Vec<(Game, Vec<Game>)>) {
    let mut result = MergeResult::default();
    let mut undo = Vec::new();
    for item in repeated {
        let positions: Vec<usize> = games.iter().enumerate().filter(|(_, g)| g.path == item.path).map(|(i, _)| i).collect();
        // La gamelist cambió desde el plan: mejor no tocar ese juego
        if positions.len() != item.entries.len() || item.keep >= positions.len() {
            continue;
        }
        let before: Vec<Game> = positions.iter().map(|&i| games[i].clone()).collect();
        let mut merged = before[item.keep].clone();
        for (i, other) in before.iter().enumerate() {
            if i != item.keep {
                fill_from(system, &mut merged, other);
            }
        }
        // De atrás adelante para que las posiciones sigan valiendo; la fusionada va en la primera
        for &i in positions.iter().skip(1).rev() {
            games.remove(i);
        }
        games[positions[0]] = merged.clone();
        result.merged += 1;
        result.removed += positions.len() - 1;
        undo.push((merged, before));
    }
    (result, undo)
}

/// Rellena lo vacío de `game` con lo de `other`; los medios, también si el de `game` no existe
/// y el de `other` sí. Lo que dice qué juego es (ids, hashes) no se copia: puede ser otro juego
fn fill_from(system: &SystemRef, game: &mut Game, other: &Game) {
    fill_empty(system, game, other, false);
}

/// Como `fill_from`; con `copy_identity`, también los ids y hashes vacíos (al migrar: es la
/// misma ROM en las dos gamelists). Devuelve si cambió algo.
pub(super) fn fill_empty(system: &SystemRef, game: &mut Game, other: &Game, copy_identity: bool) -> bool {
    let before = serde_json::to_value(&*game).ok();
    let media_fields: Vec<&str> = media::media_fields().collect();
    for field in &media_fields {
        let exists = |g: &Game| media::get_field(g, field).is_some_and(|v| !v.trim().is_empty() && media::resolve_media_path(system, v).is_file());
        if !exists(game) && exists(other) {
            media::set_field(game, field, media::get_field(other, field).cloned());
        }
    }
    // El resto de campos, vía JSON: así no hay que repetir aquí la lista de campos de Game
    let (Ok(Value::Object(mut mine)), Ok(Value::Object(theirs))) = (serde_json::to_value(&*game), serde_json::to_value(other)) else { return false };
    for (key, value) in theirs {
        let identity = IDENTITY_FIELDS.contains(&key.as_str()) && (key == "path" || !copy_identity);
        if identity || media_fields.contains(&key.as_str()) || is_empty(&value) {
            continue;
        }
        if mine.get(&key).is_none_or(is_empty) {
            mine.insert(key, value);
        }
    }
    if let Ok(filled) = serde_json::from_value::<Game>(Value::Object(mine)) {
        *game = filled;
    }
    serde_json::to_value(&*game).ok() != before
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        _ => false,
    }
}

/// Cuántos campos tiene rellenos (sin la ruta)
fn filled_fields(game: &Game) -> usize {
    match serde_json::to_value(game) {
        Ok(Value::Object(map)) => map.iter().filter(|(k, v)| k.as_str() != "path" && !is_empty(v)).count(),
        _ => 0,
    }
}

/// El MD5 que guardó el scraper (en minúsculas)
fn stored_md5(game: &Game) -> Option<String> {
    game.md5.as_ref().or(game.hash.as_ref()).map(|h| h.trim().to_lowercase()).filter(|h| h.len() == 32)
}

/// MD5 del fichero, leyéndolo a trozos (una ISO no cabe entera en memoria). Una carpeta, nada.
fn file_md5(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(path: &str, name: &str) -> Game {
        Game { path: path.into(), name: Some(name.into()), ..Default::default() }
    }

    fn setup() -> (tempfile::TempDir, SystemRef) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Spider-Man.bin"), b"rom de verdad").unwrap();
        std::fs::create_dir_all(dir.path().join("marquees")).unwrap();
        std::fs::write(dir.path().join("marquees/Spider-Man.png"), b"png").unwrap();
        let system = SystemRef::from_rom_dir(dir.path());
        (dir, system)
    }

    #[test]
    fn gana_la_del_md5_de_la_rom_y_la_otra_rellena_lo_que_falta() {
        let (dir, system) = setup();
        let md5 = file_md5(&dir.path().join("Spider-Man.bin")).unwrap();
        let mut a = game("./Spider-Man.bin", "Spider-Man (Acclaim)");
        a.md5 = Some("5db067a624e21e8b8a40eba74644f93c".into());
        a.marquee = Some("./marquees/Spider-Man.png".into());
        a.players = Some("1".into());
        a.desc = Some("La otra".into());
        let mut b = game("./Spider-Man.bin", "Spider-Man (SEGA)");
        b.md5 = Some(md5.to_uppercase());
        b.marquee = Some("./images/Spider-Man-marquee.png".into()); // ya no existe
        b.desc = Some("La buena".into());
        let mut games = vec![game("./Otro.bin", "Otro"), a, game("./Medio.bin", "Medio"), b];

        let plan = plan(&system, &games, None);
        assert_eq!(plan.len(), 1);
        assert_eq!((plan[0].keep, plan[0].reason.as_str()), (1, "md5"));

        let (result, undo) = merge(&system, &mut games, &plan);
        assert_eq!((result.merged, result.removed), (1, 1));
        assert_eq!(games.len(), 3);
        let merged = &games[1];
        assert_eq!(merged.name.as_deref(), Some("Spider-Man (SEGA)"));
        assert_eq!(merged.desc.as_deref(), Some("La buena"));
        assert_eq!(merged.md5.as_deref(), Some(md5.to_uppercase().as_str()));
        // El medio roto se cambia por el que existe; lo vacío se rellena
        assert_eq!(merged.marquee.as_deref(), Some("./marquees/Spider-Man.png"));
        assert_eq!(merged.players.as_deref(), Some("1"));
        assert_eq!(undo[0].1.len(), 2);
    }

    #[test]
    fn sin_md5_gana_la_mas_completa_y_si_empatan_la_primera() {
        let (_dir, system) = setup();
        let a = game("./Spider-Man.bin", "Uno");
        let mut b = game("./Spider-Man.bin", "Dos");
        b.genre = Some("Plataformas".into());
        let plan1 = plan(&system, &[a.clone(), b], None);
        assert_eq!((plan1[0].keep, plan1[0].reason.as_str()), (1, "complete"));
        let plan2 = plan(&system, &[a.clone(), game("./Spider-Man.bin", "Tres")], None);
        assert_eq!((plan2[0].keep, plan2[0].reason.as_str()), (0, "first"));
        // Con `only`, solo esas rutas
        assert!(plan(&system, &[a.clone(), a], Some(&["./Otro.bin".to_string()])).is_empty());
    }
}
