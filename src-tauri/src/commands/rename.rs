use std::fs;
use std::path::Path;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::layout::{Layout, SystemRef};
use crate::media;
use crate::models::GameList;
use crate::rename_rules::name_parts::{self, TagKeep};
use crate::rename_rules::titles::{self, ArticleMode};
use crate::rename_rules::CompiledRules;

use super::progress::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameEntry {
    /// Current relative ROM path (e.g. "./Super Mario World.sfc")
    pub current_path: String,
    /// Current filename without extension
    pub current_name: String,
    /// Game title from gamelist.xml (None if no title)
    pub game_title: Option<String>,
    /// Proposed new filename (sanitized title, or the name cleaned by the rules)
    pub proposed_name: String,
    /// Título propuesto para ese nombre (`rename_rules::titles`), ya estable: lo que quedaría
    /// tras repetir la propuesta hasta que no cambie. None = el título no se toca
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_title: Option<String>,
    /// Which source gave the name: "dat" | "scraper" | "goodtools" | "title", or "current" (none did:
    /// the current name, only cleaned by the rules and the tag boxes)
    #[serde(default)]
    pub source: String,
    /// Whether a rename makes sense (there is a proposal and it differs from the current name)
    pub can_rename: bool,
    /// Tracks of a .cue/.gdi renamed along with it (the ones named after the game)
    #[serde(default)]
    pub tracks: usize,
    /// Why the proposal can't be applied as is: "duplicate" (another ROM would get the
    /// same name) or "exists" (a different file already has that name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
    /// «Scraper» source: ROM not asked yet (or the search was stopped before it); «Search» asks
    #[serde(default)]
    pub pending: bool,
    /// «Scraper» source: the scraper didn't answer for this ROM (timeout…); «Search» retries it
    #[serde(default)]
    pub unanswered: bool,
}

/// Fuentes del nombre de fichero, en el orden que elige el usuario: gana la primera que da nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    /// Official name from the system's DAT, for ROMs recognised by content (CRC in the cache)
    Dat,
    /// File name a scraper has for the ROM, recognised by content (`RenameOptions::scraper` says
    /// which one; today only "screenscraper": from the cache, asked only with «Search»)
    Scraper,
    /// GoodTools codes translated to No-Intro, keeping the region ((U) → (USA), [h1] → (Hack)…);
    /// only when the name has GoodTools codes
    Goodtools,
    /// «Gamelist data»: the gamelist title plus its region and languages as No-Intro tags
    /// (`Super Mario World (USA) (En)`); keeps the id "title" so saved options stay valid
    Title,
}

/// Todo lo que se elige en la pantalla de Renombrar para proponer el nombre de fichero y el título
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameOptions {
    /// Fuentes activas, por prioridad
    #[serde(default)]
    pub sources: Vec<NameSource>,
    /// Qué scraper usa la fuente `Scraper`
    #[serde(default)]
    pub scraper: Option<String>,
    /// Reglas de limpieza sobre el nombre (las etiquetas las deciden las casillas `keep`)
    #[serde(default)]
    pub rules: bool,
    #[serde(default)]
    pub keep: TagKeep,
    /// Dónde va el artículo del título propuesto
    #[serde(default)]
    pub article: ArticleMode,
    /// «Renombrar ficheros» / «Cambiar títulos»: lo apagado no cambia al repetir la propuesta
    #[serde(default = "yes")]
    pub rename_files: bool,
    #[serde(default = "yes")]
    pub change_titles: bool,
}

fn yes() -> bool {
    true
}

/// Vueltas como mucho al repetir la propuesta (en la práctica bastan dos o tres)
const MAX_PROPOSE_ROUNDS: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct RenameResult {
    pub gamelist: GameList,
    pub renamed: usize,
    /// Games whose title changed
    #[serde(default)]
    pub titled: usize,
    /// ROMs not renamed (source missing or destination taken)
    pub skipped: Vec<String>,
}

/// Número de recopilatorio delante del nombre: 2-4 cifras y un espacio, salvo años 19xx/20xx
/// (`0004 Green Corps`, `546 Super Spy Hunter`; no `1943 The Battle of Midway` ni `1942`).
/// Es la misma condición que la regla «quitar el número del principio» de `default_rules.json`.
static LEADING_NUMBER: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^(?:\d{2,3}|[03-9]\d{3}|1[0-8]\d{2}|2[1-9]\d{2})\s+").unwrap());

/// El número de delante con su espacio («0004 »), si lo lleva
pub(super) fn leading_number(name: &str) -> Option<&str> {
    LEADING_NUMBER.find(name).filter(|m| m.end() < name.len()).map(|m| m.as_str())
}

/// Si el fichero perdió su número al renombrarlo y el título empieza por ese mismo número, el
/// título también lo pierde (solo eso: el resto del título no se toca). None = no cambia.
fn title_without_number(title: &str, old_stem: &str, new_stem: &str) -> Option<String> {
    let number = leading_number(old_stem)?;
    if new_stem.starts_with(number) {
        return None;
    }
    let rest = title.strip_prefix(number)?;
    (!rest.trim().is_empty()).then(|| rest.to_string())
}

/// Characters not allowed in filenames (on any of the usual systems).
fn is_forbidden(c: char) -> bool {
    matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

/// Characters not allowed in filenames are replaced by a hyphen. A title separator like
/// "Contra III: The Alien Wars" becomes "Contra III - The Alien Wars" (not "Contra III_ The…").
fn sanitize_filename(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len());
    for (i, &c) in chars.iter().enumerate() {
        if !is_forbidden(c) {
            out.push(c);
            continue;
        }
        // "palabra: siguiente" → "palabra - siguiente" (espacio antes del guion si falta)
        let followed_by_space = chars.get(i + 1).is_some_and(|n| n.is_whitespace());
        if followed_by_space && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
        out.push('-');
    }
    // Sin espacios dobles ni guiones colgando al principio o al final
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c: char| c == '-' || c.is_whitespace()).to_string()
}

/// Vista previa de Renombrar: para cada juego (todos, o solo `game_paths`) el nombre de fichero
/// propuesto según `options`. El scraper solo se pregunta con `ask_scraper` (botón «Buscar»,
/// gasta cupo); si no, vale lo que haya en su caché y lo demás sale como pendiente.
#[tauri::command(async)]
pub fn rename_preview(
    app: tauri::AppHandle,
    system_path: String,
    options: RenameOptions,
    game_paths: Option<Vec<String>>,
    ask_scraper: bool,
    task: Option<String>,
) -> Result<Vec<RenameEntry>, String> {
    // Todo el sistema solo con la caché del scraper (el estado, la ventana al abrirse): de lo
    // guardado si nada cambió, por opciones. Preguntar al scraper o unos juegos, siempre al momento.
    if ask_scraper || game_paths.is_some() {
        return rename_preview_now(app, system_path, options, game_paths, ask_scraper, task);
    }
    let extra = serde_json::to_string(&options).unwrap_or_default();
    let (a, sp) = (app.clone(), system_path.clone());
    super::system_status::remember(&app, &system_path, "rename", &extra, move || rename_preview_now(a, sp, options, None, false, task))
}

/// Lo que hace `rename_preview` cuando no hay nada guardado que valga
fn rename_preview_now(
    app: tauri::AppHandle,
    system_path: String,
    options: RenameOptions,
    game_paths: Option<Vec<String>>,
    ask_scraper: bool,
    task: Option<String>,
) -> Result<Vec<RenameEntry>, String> {
    let settings = crate::commands::settings::get_settings(app.clone())?;
    let (layout, system) = super::system_context(&app, &system_path)?;
    let progress = Progress::new(&app, task);
    let gamelist = crate::commands::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    let rules = CompiledRules::new(&settings.rename_rules_or_default());
    let wanted: Option<HashSet<&str>> = game_paths.as_ref().map(|paths| paths.iter().map(String::as_str).collect());
    let chosen: Vec<_> = gamelist.games.iter().filter(|g| wanted.as_ref().is_none_or(|w| w.contains(g.path.as_str()))).collect();
    let rom_of = |game: &crate::models::Game| system.rom_dir.join(game.path.strip_prefix("./").unwrap_or(&game.path));
    let uses = |source: NameSource| options.sources.contains(&source);

    // «Nombre del DAT»: el del juego que el DAT reconoce por contenido (solo lo que hay en caché)
    let dat_names = if uses(NameSource::Dat) {
        let roms: Vec<_> = chosen.iter().map(|g| rom_of(g)).collect();
        crate::commands::dat::names_by_content(&app, &system, &roms)?
    } else {
        HashMap::new()
    };
    // «Scraper»: de la caché, o preguntando por lo que falta si se pulsó «Buscar»
    let scraped = match (uses(NameSource::Scraper), options.scraper.as_deref()) {
        (false, _) => Default::default(),
        (true, Some("screenscraper")) if ask_scraper => crate::commands::ss_rom_names::names_for(&app, &settings, &system, &chosen, &progress)?,
        (true, Some("screenscraper")) => crate::commands::ss_rom_names::cached_names(&app, &system, &chosen)?,
        (true, other) => return Err(format!("Unknown scraper for file names: {}", other.unwrap_or("none"))),
    };

    let mut entries: Vec<RenameEntry> = Vec::with_capacity(chosen.len());
    for (i, game) in chosen.iter().enumerate() {
        // Tras parar al scraper, lo que queda es local y rápido: se termina la lista
        if !scraped.stopped {
            progress.check()?;
        }
        progress.step("games", i + 1, chosen.len(), &game.path);
        let found = Found {
            dat: dat_names.get(&rom_of(game)).map(String::as_str),
            scraper: scraped.names.get(&game.path).map(String::as_str),
        };
        let mut entry = propose(game, &options, &rules, &found);
        entry.pending = scraped.pending.contains(&game.path);
        entry.unanswered = scraped.failed.contains(&game.path);
        entries.push(entry);
    }
    progress.phase("checking");
    mark_conflicts(&mut entries, &system.rom_dir);
    mark_track_conflicts(&mut entries, &system.rom_dir);

    entries.sort_by(|a, b| a.current_name.to_lowercase().cmp(&b.current_name.to_lowercase()));
    Ok(entries)
}

/// Los nombres que las fuentes de fuera (DAT, scraper) tienen para un juego, sin extensión
#[derive(Default)]
struct Found<'a> {
    dat: Option<&'a str>,
    scraper: Option<&'a str>,
}

/// El nombre de la primera fuente que lo da (y cuál fue), o el actual con "current"
fn winning_name(game: &crate::models::Game, current: &str, sources: &[NameSource], found: &Found) -> (String, &'static str) {
    for source in sources {
        let name = match source {
            NameSource::Dat => found.dat.map(str::to_string),
            NameSource::Scraper => found.scraper.map(str::to_string),
            // Solo si el nombre lleva códigos de GoodTools (si no, sale igual y no cuenta)
            NameSource::Goodtools => Some(crate::rename_rules::goodtools::to_no_intro(current)).filter(|n| n != current),
            // «Datos de la gamelist»: título + región + idiomas (y, si la gamelist no los tiene, las
            // etiquetas del fichero actual)
            NameSource::Title => game
                .name
                .as_deref()
                .map(|t| name_parts::with_current_tags(&name_parts::gamelist_name(t, game.region.as_deref(), game.lang.as_deref()), current)),
        };
        if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
            let id = match source {
                NameSource::Dat => "dat",
                NameSource::Scraper => "scraper",
                NameSource::Goodtools => "goodtools",
                NameSource::Title => "title",
            };
            return (name, id);
        }
    }
    (current.to_string(), "current")
}

/// Nombre propuesto: el de la fuente ganadora, con las reglas sobre el nombre y solo las
/// etiquetas de los tipos marcados
fn compose(name: &str, options: &RenameOptions, rules: &CompiledRules) -> String {
    let (base, tags) = name_parts::split(name);
    let base = if options.rules { rules.apply(base) } else { base.to_string() };
    let tags = name_parts::filter_tags(tags, &options.keep);
    sanitize_filename(&format!("{} {}", base.trim(), tags))
}

/// Propuesta de un juego, repetida sobre su propio resultado hasta que deja de cambiar. El
/// nombre de fichero puede salir del título («Datos de la gamelist») y el título sale del nombre
/// de fichero, así que una vuelta sola a veces no basta: el artículo que se mueve al final del
/// título, GoodTools que deja paso a la gamelist, las etiquetas que cambian de orden… Antes eso
/// obligaba a renombrar dos o tres veces seguidas. Lo del DAT y del scraper va por el contenido
/// de la ROM, así que sirve igual en todas las vueltas.
fn propose(game: &crate::models::Game, options: &RenameOptions, rules: &CompiledRules, found: &Found) -> RenameEntry {
    let first = propose_once(game, options, rules, found);
    let mut state = game.clone();
    let mut last = first.clone();
    for round in 1..=MAX_PROPOSE_ROUNDS {
        let file_changes = options.rename_files && last.can_rename;
        let title_changes = last.proposed_title.is_some();
        if !file_changes && !title_changes {
            break;
        }
        // Como si ya se hubiera aplicado: el fichero nuevo (misma carpeta y extensión) y el título
        if file_changes {
            state.path = with_stem(&state.path, &last.proposed_name);
        }
        if title_changes {
            state.name = last.proposed_title.clone();
        }
        if round < MAX_PROPOSE_ROUNDS {
            last = propose_once(&state, options, rules, found);
        }
    }
    // Lo que se enseña es el juego como está ahora, con el nombre y el título del final
    let proposed_name = Path::new(&state.path).file_stem().and_then(|s| s.to_str()).unwrap_or(&first.current_name).to_string();
    let proposed_title = state.name.clone().filter(|t| options.change_titles && !t.trim().is_empty() && Some(t) != game.name.as_ref());
    RenameEntry {
        can_rename: proposed_name != first.current_name && Path::new(&game.path).extension().is_some(),
        proposed_name,
        proposed_title,
        ..first
    }
}

/// La misma ruta con otro nombre de fichero (misma carpeta y extensión)
fn with_stem(path: &str, stem: &str) -> String {
    let p = Path::new(path);
    let file = match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem.to_string(),
    };
    p.with_file_name(file).to_string_lossy().replace('\\', "/")
}

/// Una vuelta: nombre de la fuente ganadora, compuesto, y el título que saldría de él
fn propose_once(game: &crate::models::Game, options: &RenameOptions, rules: &CompiledRules, found: &Found) -> RenameEntry {
    let rom_path = Path::new(&game.path);
    let ext = rom_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let current_name = rom_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&game.path)
        .to_string();

    let (winner, source) = winning_name(game, &current_name, &options.sources, found);
    let proposed = Some(compose(&winner, options, rules))
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| current_name.clone());

    let can_rename = proposed != current_name && !ext.is_empty();
    // El fichero con el que se queda (si no se renombran ficheros, el de ahora)
    let file = if options.rename_files && !ext.is_empty() { &proposed } else { &current_name };
    let title = options
        .change_titles
        .then(|| titles::proposed_title(file, game.name.as_deref(), options.article, source != "current", &current_name))
        .filter(|t| !t.trim().is_empty() && Some(t.as_str()) != game.name.as_deref());

    RenameEntry {
        current_path: game.path.clone(),
        current_name,
        game_title: game.name.clone(),
        proposed_name: proposed,
        proposed_title: title,
        source: source.to_string(),
        can_rename,
        tracks: 0,
        conflict: None,
        pending: false,
        unanswered: false,
    }
}

/// Flag proposals that can't be applied: two ROMs ending with the same name, or a
/// different file already using it. Compared case-insensitively (macOS/Windows disks are).
fn mark_conflicts(entries: &mut [RenameEntry], rom_dir: &Path) {
    let target_of = |e: &RenameEntry| -> String {
        let current = Path::new(e.current_path.strip_prefix("./").unwrap_or(&e.current_path));
        let ext = current.extension().and_then(|x| x.to_str()).unwrap_or("");
        current
            .with_file_name(format!("{}.{}", e.proposed_name, ext))
            .to_string_lossy()
            .to_lowercase()
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    for e in entries.iter() {
        // ROMs that keep their name also "occupy" it
        let key = if e.can_rename { target_of(e) } else { e.current_path.trim_start_matches("./").to_lowercase() };
        *counts.entry(key).or_default() += 1;
    }

    for e in entries.iter_mut().filter(|e| e.can_rename) {
        let target = target_of(e);
        let own = e.current_path.trim_start_matches("./").to_lowercase();
        if counts.get(&target).copied().unwrap_or(0) > 1 {
            e.conflict = Some("duplicate".into());
        } else if target != own && rom_dir.join(&target).exists() {
            e.conflict = Some("exists".into());
        }
    }
}

/// Can a name typed by hand be used for this ROM? None if so, or why not: "invalid" (empty or
/// with characters not allowed in file names) or "exists" (another file already has that name).
fn target_conflict(rom_dir: &Path, game_path: &str, new_name: &str) -> Option<String> {
    // Vacío, con caracteres prohibidos o con espacios al principio/final
    if new_name.trim().is_empty() || new_name.trim() != new_name || new_name.chars().any(is_forbidden) {
        return Some("invalid".into());
    }
    let mut entry = [RenameEntry {
        current_path: game_path.to_string(),
        current_name: String::new(),
        game_title: None,
        proposed_name: new_name.to_string(),
        proposed_title: None,
        source: String::new(),
        can_rename: true,
        tracks: 0,
        conflict: None,
        pending: false,
        unanswered: false,
    }];
    mark_conflicts(&mut entry, rom_dir);
    mark_track_conflicts(&mut entry, rom_dir);
    entry[0].conflict.take()
}

/// Una pista de un .cue/.gdi que se renombra con él
#[derive(Debug, Clone, PartialEq)]
struct TrackRename {
    from: std::path::PathBuf,
    to: std::path::PathBuf,
    /// Cómo está escrita en la hoja y cómo quedará
    old_ref: String,
    new_ref: String,
}

/// Pistas de un .cue/.gdi cuyo nombre empieza por el nombre antiguo del juego (seguido del
/// final, un espacio, un punto o un paréntesis): «Juego (USA) (Track 1).bin» → «Juego (Track 1).bin».
/// Las de nombre genérico (track01.bin) no. Si la hoja no se lee como UTF-8, ninguna.
fn track_renames(rom: &Path, old_stem: &str, new_stem: &str) -> Vec<TrackRename> {
    if !matches!(media::extension_lower(rom).as_deref(), Some("cue" | "gdi")) || old_stem == new_stem || old_stem.is_empty() {
        return Vec::new();
    }
    let Some(dir) = rom.parent() else { return Vec::new() };
    super::move_games::sheet_names(rom)
        .into_iter()
        .filter_map(|old_ref| {
            let cut = old_ref.rfind(['/', '\\']).map_or(0, |i| i + 1);
            let (folder, file) = old_ref.split_at(cut);
            let rest = file.strip_prefix(old_stem)?;
            if !(rest.is_empty() || rest.starts_with([' ', '.', '('])) {
                return None;
            }
            let new_ref = format!("{}{}{}", folder, new_stem, rest);
            let from = dir.join(old_ref.replace('\\', "/"));
            from.is_file().then(|| TrackRename { to: dir.join(new_ref.replace('\\', "/")), from, old_ref, new_ref })
        })
        .collect()
}

/// Cuenta las pistas de cada propuesta y marca conflicto si el nombre nuevo de alguna ya existe
fn mark_track_conflicts(entries: &mut [RenameEntry], rom_dir: &Path) {
    for e in entries.iter_mut().filter(|e| e.can_rename) {
        let rom = rom_dir.join(e.current_path.strip_prefix("./").unwrap_or(&e.current_path));
        let old_stem = rom.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let tracks = track_renames(&rom, &old_stem, &e.proposed_name);
        e.tracks = tracks.len();
        if e.conflict.is_none() && tracks.iter().any(|t| t.to.exists() && t.to != t.from) {
            e.conflict = Some("exists".into());
        }
    }
}

/// Cambia en una línea de la hoja el nombre antiguo de una pista por el nuevo: entre comillas,
/// o como palabra suelta (sin comillas)
fn replace_ref(line: &str, old: &str, new: &str) -> String {
    let quoted = format!("\"{}\"", old);
    if line.contains(&quoted) {
        return line.replacen(&quoted, &format!("\"{}\"", new), 1);
    }
    let words: Vec<&str> = line.split(' ').collect();
    if words.contains(&old) {
        return words.iter().map(|w| if *w == old { new } else { *w }).collect::<Vec<_>>().join(" ");
    }
    line.to_string()
}

/// Reescribe la hoja (.cue/.gdi) con los nombres nuevos de las pistas, conservando el resto
/// (y los finales de línea). Apunta el contenido de antes para «Deshacer».
fn rewrite_sheet(sheet: &Path, tracks: &[TrackRename]) {
    let Ok(content) = fs::read_to_string(sheet) else { return };
    let mut out = String::with_capacity(content.len());
    for piece in content.split_inclusive('\n') {
        let (line, ending) = match piece.strip_suffix("\r\n") {
            Some(l) => (l, "\r\n"),
            None => match piece.strip_suffix('\n') {
                Some(l) => (l, "\n"),
                None => (piece, ""),
            },
        };
        let mut changed = line.to_string();
        for t in tracks {
            changed = replace_ref(&changed, &t.old_ref, &t.new_ref);
        }
        out.push_str(&changed);
        out.push_str(ending);
    }
    if out != content {
        media::journal_rewrite(sheet, content.into_bytes());
        let _ = fs::write(sheet, out);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameRequest {
    pub current_path: String,
    /// New file name without extension (the current one = the file stays as it is)
    pub new_name: String,
    /// New gamelist title (None = keep it, except a compilation number the file loses)
    #[serde(default)]
    pub new_title: Option<String>,
}

/// Rename ROM files and update all references in gamelist.xml.
/// Media are renamed too: the unified `<folder>/<ROM name>.<ext>` files in both structures,
/// plus (Classic) any other file the gamelist points to that is named after the ROM.
#[tauri::command(async)]
pub fn rename_roms(
    app: tauri::AppHandle,
    system_path: String,
    renames: Vec<RenameRequest>,
    task: Option<String>,
) -> Result<RenameResult, String> {
    let (layout, system) = super::system_context(&app, &system_path)?;
    let sys_path = system.rom_dir.clone();
    let progress = Progress::new(&app, task);

    // write_gamelist() backs the gamelist up before saving
    let mut gamelist = crate::commands::gamelist::read_gamelist_with(&layout, &system, &progress)?;
    // Para «Deshacer»: la gamelist de antes y el diario de ficheros movidos
    let before = gamelist.clone();
    media::journal_begin();
    let mut done: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    // Juegos a los que solo se les cambia el título (el fichero se queda como está)
    let mut retitled: Vec<(String, String)> = Vec::new();
    let mut titled = 0;

    for (i, req) in renames.iter().enumerate() {
        // Cancelado: se para aquí y se guarda lo ya renombrado (Deshacer lo cubre)
        if progress.cancelled() {
            break;
        }
        progress.step("renaming", i + 1, renames.len(), &req.current_path);
        let old_rel = req.current_path.strip_prefix("./").unwrap_or(&req.current_path);
        let old_rom = Path::new(old_rel);
        let ext = old_rom
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let old_stem = old_rom
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Solo el título: el fichero no se toca
        if req.new_name == old_stem {
            if let Some(game) = gamelist.games.iter_mut().find(|g| g.path == req.current_path) {
                if set_title(game, req.new_title.as_deref()) {
                    titled += 1;
                    retitled.push((req.current_path.clone(), req.current_path.clone()));
                }
            }
            continue;
        }
        // Un nombre escrito a mano con caracteres no válidos (o vacío) no se aplica
        if target_conflict(&sys_path, &req.current_path, &req.new_name).as_deref() == Some("invalid") {
            skipped.push(old_rel.to_string());
            continue;
        }

        // Keep the ROM in its own subfolder (only the file name changes)
        let new_filename = format!("{}.{}", req.new_name, ext);
        let new_rel = match old_rom.parent().filter(|p| !p.as_os_str().is_empty()) {
            Some(parent) => parent.join(&new_filename).to_string_lossy().replace('\\', "/"),
            None => new_filename.clone(),
        };
        let new_relative = format!("./{}", new_rel);

        // Resolve absolute paths
        let old_abs = sys_path.join(old_rel);
        let new_abs = sys_path.join(&new_rel);

        // Skip if source doesn't exist or destination already exists
        if !old_abs.exists() || (new_abs.exists() && old_abs != new_abs) {
            skipped.push(old_rel.to_string());
            continue;
        }
        let rom_is_dir = old_abs.is_dir();

        // Pistas de un .cue/.gdi con el nombre del juego: si alguna chocaría, el juego se salta
        let tracks = track_renames(&old_abs, old_stem, &req.new_name);
        if tracks.iter().any(|t| t.to.exists() && t.to != t.from) {
            skipped.push(old_rel.to_string());
            continue;
        }

        // Rename the ROM file
        if old_abs != new_abs {
            fs::rename(&old_abs, &new_abs)
                .map_err(|e| format!("Failed to rename {}: {}", old_rel, e))?;
            media::journal_move(&old_abs, &new_abs);
        }

        // …y sus pistas, con la hoja reescrita con los nombres nuevos
        let mut renamed_tracks = Vec::new();
        for t in tracks {
            if fs::rename(&t.from, &t.to).is_ok() {
                media::journal_move(&t.from, &t.to);
                renamed_tracks.push(t);
            }
        }
        if !renamed_tracks.is_empty() {
            rewrite_sheet(&new_abs, &renamed_tracks);
        }

        let moved = media::rename_rom_media(&layout, &system, &req.current_path, &new_relative, rom_is_dir);
        done.push((req.current_path.clone(), new_relative.clone()));

        // Update gamelist entry
        for game in &mut gamelist.games {
            if game.path == req.current_path {
                game.path = new_relative.clone();
                if req.new_title.is_some() {
                    if set_title(game, req.new_title.as_deref()) {
                        titled += 1;
                    }
                } else if let Some(title) = game.name.as_deref().and_then(|t| title_without_number(t, old_stem, &req.new_name)) {
                    // Recopilatorios: «0004 Green Corps» pierde el número igual que el fichero
                    game.name = Some(title);
                }

                if layout.writes_media_tags() {
                    for field in media::media_fields() {
                        update_media_field(game, field, &layout, &system, &moved, old_stem, &req.new_name);
                    }
                }
                break;
            }
        }
    }

    progress.phase("saving");
    crate::commands::gamelist::write_gamelist(&layout, &system, &gamelist)?;
    update_references(&layout, &system, &done);

    let sys = system.rom_dir.to_string_lossy().to_string();
    super::undo::record(super::undo::UndoOp {
        kind: "rename".into(),
        system: sys.clone(),
        files: media::journal_take(),
        // Deshacer también devuelve los títulos (los de los ficheros renombrados y los demás)
        games: super::undo::changes_by_path(&sys, &before, &sys, &gamelist, &[done.clone(), retitled].concat()),
        refs: super::undo::RefUndo::Rename { system: sys.clone(), pairs: done.clone() },
        ..Default::default()
    });

    Ok(RenameResult {
        // Re-read so ES-DE media fields reflect the renamed files
        gamelist: crate::commands::gamelist::read_gamelist(&layout, &system)?,
        renamed: done.len(),
        titled,
        skipped,
    })
}

/// Pone el título nuevo si lo hay y es distinto (vacío no vale). true = cambió
fn set_title(game: &mut crate::models::Game, title: Option<&str>) -> bool {
    match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) if game.name.as_deref() != Some(t) => {
            game.name = Some(t.to_string());
            true
        }
        _ => false,
    }
}

/// After renaming ROMs, fix what points to them by file name:
/// custom collections (where the active structure keeps them) and .m3u playlists.
pub(super) fn update_references(layout: &Layout, system: &SystemRef, renamed: &[(String, String)]) {
    if renamed.is_empty() {
        return;
    }
    // Game statuses are kept by ROM path: they follow the new name
    super::game_status::remap(&system.rom_dir, &system.rom_dir, renamed);
    // Y la marca de «no encontrado por el scraper», igual
    super::scrape_misses::remap(&system.rom_dir, &system.rom_dir, renamed);

    // Collections: entries are written exactly as collection_entry() builds them
    let collection_renames: HashMap<String, String> = renamed
        .iter()
        .map(|(old, new)| (layout.collection_entry(system, old), layout.collection_entry(system, new)))
        .collect();
    let collections_dir = layout.collections_dir(system);
    if let Ok(entries) = fs::read_dir(&collections_dir) {
        for path in entries.flatten().map(|e| e.path()) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with("custom-") && name.ends_with(".cfg") {
                rewrite_lines(&path, |line| collection_renames.get(line.trim()).cloned());
            }
        }
    }

    // .m3u playlists: lines are paths relative to the playlist's own folder
    let renamed_abs: HashMap<std::path::PathBuf, String> = renamed
        .iter()
        .map(|(old, new)| {
            let new_name = Path::new(new).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            (system.rom_dir.join(crate::layout::strip_dot_slash(old)), new_name)
        })
        .collect();
    for m3u in media::list_files_recursive(&system.rom_dir)
        .into_iter()
        .filter(|p| media::extension_lower(p).as_deref() == Some("m3u"))
    {
        let Some(dir) = m3u.parent().map(Path::to_path_buf) else { continue };
        rewrite_lines(&m3u, |line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let new_name = renamed_abs.get(&dir.join(trimmed))?;
            Some(Path::new(trimmed).with_file_name(new_name).to_string_lossy().replace('\\', "/"))
        });
    }
}

/// After removing games (Duplicates, Games without ROM): drop the lines that pointed to their
/// ROMs from the custom collections and from the .m3u playlists of the system folder.
/// A playlist left without discs stays (empty), it isn't deleted.
pub(super) fn remove_references(layout: &Layout, system: &SystemRef, removed: &[String]) {
    if removed.is_empty() {
        return;
    }
    super::game_status::forget(&system.rom_dir, removed);
    super::scrape_misses::forget(&system.rom_dir, removed);

    let entries: std::collections::HashSet<String> = removed.iter().map(|p| layout.collection_entry(system, p)).collect();
    if let Ok(files) = fs::read_dir(layout.collections_dir(system)) {
        for path in files.flatten().map(|e| e.path()) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with("custom-") && name.ends_with(".cfg") {
                remove_lines(&path, |line| entries.contains(line.trim()));
            }
        }
    }

    let removed_abs: std::collections::HashSet<std::path::PathBuf> =
        removed.iter().map(|p| system.rom_dir.join(crate::layout::strip_dot_slash(p))).collect();
    for m3u in media::list_files_recursive(&system.rom_dir)
        .into_iter()
        .filter(|p| media::extension_lower(p).as_deref() == Some("m3u"))
    {
        let Some(dir) = m3u.parent().map(Path::to_path_buf) else { continue };
        remove_lines(&m3u, |line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && removed_abs.contains(&dir.join(trimmed))
        });
    }
}

/// Remove the lines of a text file for which `drop` is true (keeping its line endings).
/// Only writes if something was removed.
pub(super) fn remove_lines(path: &Path, drop: impl Fn(&str) -> bool) {
    let Ok(content) = fs::read_to_string(path) else { return };
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = content.lines().filter(|l| !drop(l)).collect();
    if kept.len() == content.lines().count() {
        return;
    }
    let mut out = kept.join(newline);
    if !kept.is_empty() && content.ends_with('\n') {
        out.push_str(newline);
    }
    let _ = fs::write(path, out);
}

/// Rewrite a text file line by line (keeping its line endings); `change` returns the
/// new content for a line, or None to leave it as is. Only writes if something changed.
pub(super) fn rewrite_lines(path: &Path, change: impl Fn(&str) -> Option<String>) {
    let Ok(content) = fs::read_to_string(path) else { return };
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut changed = false;
    let lines: Vec<String> = content
        .lines()
        .map(|line| match change(line) {
            Some(new) if new != line => {
                changed = true;
                new
            }
            _ => line.to_string(),
        })
        .collect();
    if changed {
        let mut out = lines.join(newline);
        if content.ends_with('\n') {
            out.push_str(newline);
        }
        let _ = fs::write(path, out);
    }
}

/// Classic: point a media field to its renamed file. Unified files were already moved
/// (`moved`); anything else named after the ROM (legacy suffixes) is renamed here.
fn update_media_field(
    game: &mut crate::models::Game,
    field: &str,
    layout: &crate::layout::Layout,
    system: &crate::layout::SystemRef,
    moved: &[(std::path::PathBuf, std::path::PathBuf)],
    old_stem: &str,
    new_stem: &str,
) {
    let Some(current) = media::get_field(game, field).cloned() else { return };
    let current_abs = media::resolve_media_path(system, &current);

    if let Some((_, new)) = moved.iter().find(|(old, _)| *old == current_abs) {
        media::set_field(game, field, Some(media::media_ref(layout, system, new)));
        return;
    }

    let mut value = Some(current);
    rename_media_field(&mut value, &system.rom_dir, old_stem, new_stem);
    media::set_field(game, field, value);
}

/// Rename a media file if its stem matches the old ROM stem.
fn rename_media_field(
    field: &mut Option<String>,
    sys_path: &Path,
    old_stem: &str,
    new_stem: &str,
) {
    let Some(media_path) = field.as_ref() else {
        return;
    };

    let media_rel = media_path.strip_prefix("./").unwrap_or(media_path);
    let media_p = Path::new(media_rel);

    let media_filestem = media_p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Check if the media filename starts with the old ROM stem
    // Handles both "Mario.png" and "Mario-image.png" patterns
    if !media_filestem.starts_with(old_stem) {
        return;
    }

    let suffix = &media_filestem[old_stem.len()..]; // e.g. "", "-image", "-box-2D"
    let media_ext = media_p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let parent = media_p.parent().and_then(|p| p.to_str()).unwrap_or("");

    let new_media_filename = format!("{}{}.{}", new_stem, suffix, media_ext);
    let new_media_rel = if parent.is_empty() {
        new_media_filename.clone()
    } else {
        format!("{}/{}", parent, new_media_filename)
    };

    let old_abs = sys_path.join(media_rel);
    let new_abs = sys_path.join(&new_media_rel);

    if old_abs.exists() && old_abs != new_abs {
        if fs::rename(&old_abs, &new_abs).is_ok() {
            media::journal_move(&old_abs, &new_abs);
            *field = Some(format!("./{}", new_media_rel));
        }
    } else {
        // File might not exist, just update the path reference
        *field = Some(format!("./{}", new_media_rel));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Game;
    use crate::rename_rules::default_rules;

    fn options(sources: &[NameSource], rules: bool) -> RenameOptions {
        RenameOptions { sources: sources.to_vec(), scraper: None, rules, keep: TagKeep::default(), article: ArticleMode::Keep, rename_files: true, change_titles: true }
    }

    fn propose_with(game: &Game, opts: &RenameOptions, found: &Found) -> RenameEntry {
        propose(game, opts, &CompiledRules::new(&default_rules()), found)
    }

    /// Una pasada deja lo que dejarían varias: lo propuesto ya no cambia al volver a proponer
    fn assert_stable(game: &Game, opts: &RenameOptions) -> RenameEntry {
        let e = propose_with(game, opts, &Found::default());
        let mut after = game.clone();
        let ext = Path::new(&game.path).extension().and_then(|x| x.to_str()).unwrap_or("");
        after.path = format!("./{}.{}", e.proposed_name, ext);
        if let Some(t) = &e.proposed_title {
            after.name = Some(t.clone());
        }
        let again = propose_with(&after, opts, &Found::default());
        assert!(!again.can_rename && again.proposed_title.is_none(), "{e:?} -> {again:?}");
        e
    }

    #[test]
    fn una_pasada_basta_con_el_articulo() {
        let opts = RenameOptions { article: ArticleMode::End, ..options(&[NameSource::Title], false) };
        let mut game = Game { path: "./lkage.zip".into(), name: Some("The Legend of Kage".into()), ..Default::default() };
        game.region = Some("us".into());
        game.lang = Some("en".into());
        let e = assert_stable(&game, &opts);
        assert_eq!(e.proposed_name, "Legend of Kage, The (USA) (En)");
        assert_eq!(e.proposed_title.as_deref(), Some("Legend of Kage, The"));
    }

    #[test]
    fn una_pasada_basta_con_goodtools_y_la_gamelist() {
        let opts = options(&[NameSource::Goodtools, NameSource::Title], false);
        let mut game = Game { path: "./Wild Card (J) [!].wsc".into(), name: Some("Wild Card".into()), ..Default::default() };
        game.region = Some("jp".into());
        game.lang = Some("ja".into());
        let e = assert_stable(&game, &opts);
        assert_eq!(e.proposed_name, "Wild Card (Japan) (Ja)");
        assert_eq!(e.proposed_title, None);
    }

    #[test]
    fn con_los_ficheros_apagados_solo_cambia_el_titulo() {
        let opts = RenameOptions { article: ArticleMode::End, rename_files: false, ..options(&[NameSource::Title], false) };
        let game = Game { path: "./The Firemen.zip".into(), name: Some("The Firemen".into()), ..Default::default() };
        let e = propose_with(&game, &opts, &Found::default());
        assert!(!e.can_rename);
        assert_eq!(e.proposed_title.as_deref(), Some("Firemen, The"));
    }

    /// Solo las reglas, como la antigua fuente «Reglas» (todas las etiquetas fuera)
    fn entry_for(path: &str) -> RenameEntry {
        let game = Game { path: path.into(), name: Some("Titulo".into()), ..Default::default() };
        let none = TagKeep { region: false, language: false, revision: false, status: false, dump: false, other: false };
        propose_with(&game, &RenameOptions { keep: none, ..options(&[], true) }, &Found::default())
    }

    #[test]
    fn the_leading_number_rule_and_the_title() {
        let rules = CompiledRules::new(&default_rules());
        for (from, to) in [
            ("0004 Green Corps", "Green Corps"),
            ("546 Super Spy Hunter", "Super Spy Hunter"),
            ("002 Super Mario World", "Super Mario World"),
            ("1943 The Battle of Midway", "1943 The Battle of Midway"),
            ("1942", "1942"),
            ("10-Yard Fight", "10-Yard Fight"),
            ("2010 Street Fighter", "2010 Street Fighter"),
        ] {
            assert_eq!(rules.apply(from).trim(), to, "{}", from);
        }
        // El título pierde el mismo número que el fichero, y nada más
        assert_eq!(title_without_number("0004 Green Corps", "0004 Green Corps", "Green Corps").as_deref(), Some("Green Corps"));
        assert_eq!(title_without_number("0004 Green Corps (Chinese)", "0004 Green Corps", "Green Corps").as_deref(), Some("Green Corps (Chinese)"));
        assert_eq!(title_without_number("Green Corps", "0004 Green Corps", "Green Corps"), None, "the title had no number");
        assert_eq!(title_without_number("0005 Other", "0004 Green Corps", "Green Corps"), None, "another number");
        assert_eq!(title_without_number("0004 Green Corps", "0004 Green Corps", "0004 Green Corps (USA)"), None, "the file keeps it");
        assert_eq!(title_without_number("1943 Midway", "1943 Midway", "Midway"), None, "years are not compilation numbers");
    }

    #[test]
    fn sources_are_read_with_the_names_the_ui_sends() {
        for (text, source) in [("title", NameSource::Title), ("dat", NameSource::Dat), ("goodtools", NameSource::Goodtools), ("scraper", NameSource::Scraper)] {
            assert_eq!(serde_json::from_str::<NameSource>(&format!("\"{}\"", text)).unwrap(), source);
        }
        let opts: RenameOptions = serde_json::from_str(r#"{"sources":["dat","scraper"],"scraper":"screenscraper","rules":true,"keep":{"dump":false}}"#).unwrap();
        assert_eq!(opts.sources, vec![NameSource::Dat, NameSource::Scraper]);
        assert!(opts.keep.region && !opts.keep.dump, "unsent boxes are kept");
    }

    #[test]
    fn the_first_source_with_a_name_wins_and_the_extension_stays() {
        // ScreenScraper conoce «Super Mario World (USA).sfc» (el de dentro del zip): llega sin
        // extensión y el fichero sigue siendo .zip
        let game = Game { path: "./mario mal.zip".into(), name: Some("Super Mario World".into()), ..Default::default() };
        let chain = options(&[NameSource::Dat, NameSource::Scraper, NameSource::Title], false);
        let e = propose_with(&game, &chain, &Found { dat: None, scraper: Some("Super Mario World (USA)") });
        assert_eq!((e.proposed_name.as_str(), e.source.as_str()), ("Super Mario World (USA)", "scraper"));
        assert!(e.can_rename);
        let e = propose_with(&game, &chain, &Found { dat: Some("Super Mario World (Europe)"), scraper: Some("Super Mario World (USA)") });
        assert_eq!((e.proposed_name.as_str(), e.source.as_str()), ("Super Mario World (Europe)", "dat"));
        // Ni DAT ni scraper: el título
        let e = propose_with(&game, &chain, &Found::default());
        assert_eq!((e.proposed_name.as_str(), e.source.as_str()), ("Super Mario World", "title"));
        // Sin fuentes: se queda como está
        let e = propose_with(&game, &options(&[NameSource::Dat], false), &Found::default());
        assert_eq!(e.source, "current");
        assert!(!e.can_rename);
    }

    #[test]
    fn goodtools_only_counts_when_the_name_has_its_codes() {
        let chain = options(&[NameSource::Goodtools, NameSource::Title], false);
        let gt = Game { path: "./Gradius (J) [!].nes".into(), name: Some("Gradius".into()), ..Default::default() };
        let e = propose_with(&gt, &chain, &Found::default());
        assert_eq!((e.proposed_name.as_str(), e.source.as_str()), ("Gradius (Japan)", "goodtools"));
        let plain = Game { path: "./Sonic (USA).md".into(), name: Some("Sonic the Hedgehog".into()), ..Default::default() };
        let e = propose_with(&plain, &chain, &Found::default());
        assert_eq!(e.source, "title", "no GoodTools codes: the next source");
    }

    #[test]
    fn rules_clean_the_name_and_the_boxes_decide_the_tags() {
        let game = Game { path: "./0004 Green Corps (USA) (En,Fr) (Rev 1) [!].nes".into(), ..Default::default() };
        // Las reglas de serie quitarían «(USA)» y «(Rev 1)»: aquí solo tocan el nombre
        let keep = TagKeep { language: false, dump: false, ..Default::default() };
        let e = propose_with(&game, &RenameOptions { keep, ..options(&[], true) }, &Found::default());
        assert_eq!(e.proposed_name, "Green Corps (USA) (Rev 1)");
        // Sin reglas y con todo marcado: igual que estaba
        let e = propose_with(&game, &options(&[], false), &Found::default());
        assert!(!e.can_rename);
    }

    #[test]
    fn typed_name_is_checked_against_disk_and_invalid_characters() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Otro.sfc"), b"x").unwrap();
        fs::write(tmp.path().join("Juego.sfc"), b"x").unwrap();
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Nuevo"), None);
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Otro").as_deref(), Some("exists"));
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Mal: nombre").as_deref(), Some("invalid"));
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "").as_deref(), Some("invalid"));
        // Keeping the same name is not a clash with itself
        assert_eq!(target_conflict(tmp.path(), "./Juego.sfc", "Juego"), None);
    }

    #[test]
    fn forbidden_characters_become_hyphens() {
        assert_eq!(sanitize_filename("Contra III: The Alien Wars"), "Contra III - The Alien Wars");
        assert_eq!(sanitize_filename("Donkey Kong Country 2: Diddy's Kong Quest"), "Donkey Kong Country 2 - Diddy's Kong Quest");
        assert_eq!(sanitize_filename("AC/DC"), "AC-DC");
        assert_eq!(sanitize_filename("Who? Me"), "Who - Me");
        assert_eq!(sanitize_filename("Juego Normal"), "Juego Normal");
    }

    #[test]
    fn rules_and_no_tags_clean_the_file_name() {
        let e = entry_for("./Super Juego (USA) (Rev 1).sfc");
        assert_eq!(e.proposed_name, "Super Juego");
        assert!(e.can_rename);
    }

    #[test]
    fn same_target_twice_is_a_duplicate_and_existing_files_are_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Otro.sfc"), b"x").unwrap();
        let mut entries = vec![
            entry_for("./Juego (USA).sfc"),
            entry_for("./Juego (Europe).sfc"),
            entry_for("./Otro (Japan).sfc"),
            entry_for("./Bien (USA).sfc"),
        ];
        mark_conflicts(&mut entries, tmp.path());
        assert_eq!(entries[0].conflict.as_deref(), Some("duplicate"));
        assert_eq!(entries[1].conflict.as_deref(), Some("duplicate"));
        assert_eq!(entries[2].conflict.as_deref(), Some("exists"));
        assert_eq!(entries[3].conflict, None);
    }

    #[test]
    fn removed_roms_leave_collections_and_m3u() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("psx"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        fs::write(system.rom_dir.join("custom-Favoritos.cfg"), "./Uno.cue\r\n./Dos.cue\r\n").unwrap();
        fs::write(system.rom_dir.join("Juego.m3u"), "Juego (Disc 1).cue\nJuego (Disc 2).cue\n").unwrap();

        remove_references(&Layout::Classic, &system, &["./Uno.cue".into(), "./Juego (Disc 2).cue".into()]);
        assert_eq!(fs::read_to_string(system.rom_dir.join("custom-Favoritos.cfg")).unwrap(), "./Dos.cue\r\n");
        assert_eq!(fs::read_to_string(system.rom_dir.join("Juego.m3u")).unwrap(), "Juego (Disc 1).cue\n");
    }

    #[test]
    fn collections_and_m3u_follow_the_renamed_roms() {
        let tmp = tempfile::tempdir().unwrap();
        let system = SystemRef::from_rom_dir(tmp.path().join("psx"));
        fs::create_dir_all(&system.rom_dir).unwrap();
        fs::write(system.rom_dir.join("custom-Favs.cfg"), "./Juego (USA) (Disc 1).cue\n./Otro.cue\n").unwrap();
        fs::write(system.rom_dir.join("Juego.m3u"), "Juego (USA) (Disc 1).cue\r\nJuego (USA) (Disc 2).cue\r\n").unwrap();

        let renamed = vec![("./Juego (USA) (Disc 1).cue".to_string(), "./Juego (Disc 1).cue".to_string())];
        update_references(&Layout::Classic, &system, &renamed);

        assert_eq!(
            fs::read_to_string(system.rom_dir.join("custom-Favs.cfg")).unwrap(),
            "./Juego (Disc 1).cue\n./Otro.cue\n"
        );
        assert_eq!(
            fs::read_to_string(system.rom_dir.join("Juego.m3u")).unwrap(),
            "Juego (Disc 1).cue\r\nJuego (USA) (Disc 2).cue\r\n"
        );
    }

    #[test]
    fn a_cue_takes_its_tracks_along_and_generic_tracks_stay() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        fs::write(d.join("Juego (USA).cue"), "FILE \"Juego (USA) (Track 1).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\nFILE \"Juego (USA) (Track 2).bin\" BINARY\r\n").unwrap();
        fs::write(d.join("Juego (USA) (Track 1).bin"), "1").unwrap();
        fs::write(d.join("Juego (USA) (Track 2).bin"), "2").unwrap();
        let tracks = track_renames(&d.join("Juego (USA).cue"), "Juego (USA)", "Juego");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].new_ref, "Juego (Track 1).bin");

        // Lo que hace rename_roms: ROM, pistas y hoja
        fs::rename(d.join("Juego (USA).cue"), d.join("Juego.cue")).unwrap();
        for t in &tracks {
            fs::rename(&t.from, &t.to).unwrap();
        }
        rewrite_sheet(&d.join("Juego.cue"), &tracks);
        assert_eq!(
            fs::read_to_string(d.join("Juego.cue")).unwrap(),
            "FILE \"Juego (Track 1).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\nFILE \"Juego (Track 2).bin\" BINARY\r\n",
            "only the names change, CRLF kept"
        );
        assert!(d.join("Juego (Track 2).bin").is_file());

        // Una pista que se llama igual que el juego
        fs::write(d.join("Uno (Europe).cue"), "FILE \"Uno (Europe).bin\" BINARY\n").unwrap();
        fs::write(d.join("Uno (Europe).bin"), "x").unwrap();
        assert_eq!(track_renames(&d.join("Uno (Europe).cue"), "Uno (Europe)", "Uno")[0].new_ref, "Uno.bin");

        // GDI con pistas genéricas: no se tocan
        fs::write(d.join("Crazy (Japan).gdi"), "2\n1 0 4 2352 track01.bin 0\n2 600 0 2352 track02.raw 0\n").unwrap();
        fs::write(d.join("track01.bin"), "").unwrap();
        fs::write(d.join("track02.raw"), "").unwrap();
        assert!(track_renames(&d.join("Crazy (Japan).gdi"), "Crazy (Japan)", "Crazy").is_empty());
    }

    #[test]
    fn a_track_clash_is_a_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        fs::write(d.join("Juego (USA).cue"), "FILE \"Juego (USA).bin\" BINARY\n").unwrap();
        fs::write(d.join("Juego (USA).bin"), "x").unwrap();
        fs::write(d.join("Juego.bin"), "otro").unwrap();
        assert_eq!(target_conflict(d, "./Juego (USA).cue", "Juego").as_deref(), Some("exists"));
        assert_eq!(target_conflict(d, "./Juego (USA).cue", "Juego Bueno"), None);
    }
}
