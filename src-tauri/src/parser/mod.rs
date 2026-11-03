use std::collections::HashMap;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use serde::{Deserialize, Serialize};

use crate::models::folder::Folder;
use crate::models::game::Game;
use crate::models::provider::Provider;
use crate::models::GameList;

/// XML-specific wrapper for gamelist.xml deserialization.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "gameList")]
struct XmlGameList {
    #[serde(rename = "provider", skip_serializing_if = "Option::is_none", default)]
    provider: Option<Provider>,
    #[serde(rename = "folder", default)]
    folder: Vec<Folder>,
    #[serde(rename = "game", default)]
    game: Vec<Game>,
}

pub fn parse_gamelist(xml: &str) -> Result<GameList, String> {
    let xml_list: XmlGameList =
        quick_xml::de::from_str(xml).map_err(|e| format!("Failed to parse gamelist.xml: {}", e))?;
    Ok(GameList {
        games: xml_list.game,
        folders: xml_list.folder,
        provider: xml_list.provider,
    })
}

/// Serialize a gamelist to XML, preserving unknown fields from the original XML.
/// If `original_xml` is provided, unknown child elements of each <game> are preserved.
/// If not provided, falls back to a clean serialization of known fields only.
pub fn serialize_gamelist(gamelist: &GameList, original_xml: Option<&str>) -> Result<String, String> {
    match original_xml {
        Some(orig) => merge_serialize(gamelist, orig),
        None => clean_serialize(gamelist),
    }
}

/// Clean serialization (no original XML to merge with).
fn clean_serialize(gamelist: &GameList) -> Result<String, String> {
    let xml_list = XmlGameList {
        provider: gamelist.provider.clone(),
        folder: gamelist.folders.clone(),
        game: gamelist.games.clone(),
    };
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let body = quick_xml::se::to_string(&xml_list)
        .map_err(|e| format!("Failed to serialize gamelist: {}", e))?;
    xml.push_str(&body);
    xml.push('\n');
    Ok(xml)
}

/// Known field names we manage in the Game struct.
const KNOWN_GAME_FIELDS: &[&str] = &[
    "path", "name", "sortname", "desc", "image", "thumbnail", "marquee",
    "fanart", "titleshot", "boxback", "boxart", "cartridge", "wheel", "mix",
    "manual", "video", "rating", "releasedate", "developer", "publisher",
    "genre", "genreid", "players", "family", "arcadesystemname", "region",
    "lang", "favorite", "hidden", "kidgame", "playcount", "lastplayed",
    "gametime", "emulator", "core", "hash", "md5", "crc32",
    "cheevosHash", "cheevosId",
];

/// Known attribute names we manage on <game> elements.
const KNOWN_GAME_ATTRS: &[&str] = &["id", "source"];

/// Root-level elements we manage; anything else under <gameList> (e.g. ES-DE's
/// <alternativeEmulator>) is preserved as raw XML.
const KNOWN_ROOT_ELEMENTS: &[&str] = &["provider", "folder", "game"];

/// Extract the raw XML of unknown direct children of <gameList>, in document order.
fn extract_root_unknowns(original_xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(original_xml);
    let mut unknowns: Vec<String> = Vec::new();
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                if depth == 2 {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if KNOWN_ROOT_ELEMENTS.contains(&tag.as_str()) {
                        skip_subtree(&mut reader)?;
                    } else {
                        let mut capture_depth = depth;
                        unknowns.push(capture_element_raw(&mut reader, e, &mut capture_depth)?);
                    }
                    // Both helpers consume the matching end tag
                    depth -= 1;
                }
            }
            Ok(Event::Empty(ref e)) if depth == 1 => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if !KNOWN_ROOT_ELEMENTS.contains(&tag.as_str()) {
                    unknowns.push(empty_element_raw(e));
                }
            }
            Ok(Event::End(_)) => depth -= 1,
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    Ok(unknowns)
}

/// Consume events until the end tag matching an already-read start tag.
fn skip_subtree(reader: &mut Reader<&[u8]>) -> Result<(), String> {
    let mut inner_depth = 1;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => inner_depth += 1,
            Ok(Event::End(_)) => {
                inner_depth -= 1;
                if inner_depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }
}

/// Raw XML for a self-closing element, keeping its attributes.
fn empty_element_raw(e: &BytesStart) -> String {
    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
    let mut raw = format!("<{}", tag);
    for attr in e.attributes().flatten() {
        let k = String::from_utf8_lossy(attr.key.as_ref());
        let v = String::from_utf8_lossy(&attr.value);
        raw.push_str(&format!(" {}=\"{}\"", k, v));
    }
    raw.push_str("/>");
    raw
}

/// Extract the raw XML of unknown child elements for each <game> in the original XML.
/// Returns a map from game path -> Vec of raw XML strings for unknown elements.
fn extract_unknown_elements(original_xml: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let mut reader = Reader::from_str(original_xml);
    let mut result: HashMap<String, Vec<String>> = HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"game" => {
                let (path, unknowns, unknown_attrs) = extract_game_unknowns(&mut reader, e)?;
                if !unknowns.is_empty() || !unknown_attrs.is_empty() {
                    let mut all = unknown_attrs;
                    all.extend(unknowns);
                    result.insert(path, all);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    Ok(result)
}

/// Parse a single <game> element, returning (path, unknown_elements, unknown_attributes_as_xml).
fn extract_game_unknowns(
    reader: &mut Reader<&[u8]>,
    start_event: &BytesStart,
) -> Result<(String, Vec<String>, Vec<String>), String> {
    let mut path = String::new();
    let mut unknowns: Vec<String> = Vec::new();
    let mut unknown_attrs: Vec<String> = Vec::new();
    let mut depth = 1;

    // Check for unknown attributes on the <game> element itself
    for attr in start_event.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        if !KNOWN_GAME_ATTRS.contains(&key.as_str()) {
            let val = String::from_utf8_lossy(&attr.value).to_string();
            // Store as a special marker so we can re-add them
            unknown_attrs.push(format!("@attr:{}={}", key, val));
        }
    }

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                depth += 1;

                if depth == 2 {
                    if tag == "path" {
                        // Read path value
                        if let Ok(Event::Text(t)) = reader.read_event() {
                            path = t.unescape().unwrap_or_default().to_string();
                        }
                        // consume </path>
                        let _ = reader.read_event();
                        depth -= 1;
                    } else if KNOWN_GAME_FIELDS.contains(&tag.as_str()) {
                        // Skip known fields entirely
                        skip_element(reader, &mut depth);
                    } else {
                        // Unknown field — capture its raw XML
                        let raw = capture_element_raw(reader, e, &mut depth)?;
                        unknowns.push(raw);
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if !KNOWN_GAME_FIELDS.contains(&tag.as_str()) && tag != "path" {
                    // Self-closing unknown element
                    unknowns.push(empty_element_raw(e));
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error in game: {}", e)),
            _ => {}
        }
    }

    Ok((path, unknowns, unknown_attrs))
}

/// Skip over a known element and all its children.
fn skip_element(reader: &mut Reader<&[u8]>, depth: &mut i32) {
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => *depth += 1,
            Ok(Event::End(_)) => {
                *depth -= 1;
                if *depth < 2 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }
}

/// Capture a complete unknown element as raw XML string.
fn capture_element_raw(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart,
    depth: &mut i32,
) -> Result<String, String> {
    let tag = String::from_utf8_lossy(start.name().as_ref()).to_string();
    let mut raw = String::from("<");
    raw.push_str(&tag);
    for attr in start.attributes().flatten() {
        let k = String::from_utf8_lossy(attr.key.as_ref());
        let v = String::from_utf8_lossy(&attr.value);
        raw.push_str(&format!(" {}=\"{}\"", k, v));
    }
    raw.push('>');

    let mut inner_depth = 1;
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                inner_depth += 1;
                let inner_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                raw.push_str(&format!("<{}", inner_tag));
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let v = String::from_utf8_lossy(&attr.value).to_string();
                    raw.push_str(&format!(" {}=\"{}\"", k, v));
                }
                raw.push('>');
            }
            Ok(Event::Empty(ref e)) => {
                let inner_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                raw.push_str(&format!("<{}", inner_tag));
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let v = String::from_utf8_lossy(&attr.value).to_string();
                    raw.push_str(&format!(" {}=\"{}\"", k, v));
                }
                raw.push_str("/>");
            }
            Ok(Event::Text(ref t)) => {
                // Re-escape: the text is written back as raw XML (e.g. "&amp;" must stay "&amp;")
                let text = t.unescape().unwrap_or_default();
                raw.push_str(&quick_xml::escape::escape(text.as_ref()));
            }
            Ok(Event::CData(ref t)) => {
                let cdata = String::from_utf8_lossy(t.as_ref()).to_string();
                raw.push_str(&format!("<![CDATA[{}]]>", cdata));
            }
            Ok(Event::End(ref e)) => {
                inner_depth -= 1;
                let end_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                raw.push_str(&format!("</{}>", end_tag));
                if inner_depth == 0 {
                    *depth -= 1;
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error capturing element: {}", e)),
            _ => {}
        }
    }

    Ok(raw)
}

/// Merge-serialize: combine our known GameList data with unknown elements from original XML.
fn merge_serialize(gamelist: &GameList, original_xml: &str) -> Result<String, String> {
    let unknown_map = extract_unknown_elements(original_xml)?;
    let root_unknowns = extract_root_unknowns(original_xml)?;

    // Build output using quick-xml writer for proper escaping
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| format!("Write error: {}", e))?;

    // <gameList>
    writer
        .write_event(Event::Start(BytesStart::new("gameList")))
        .map_err(|e| format!("Write error: {}", e))?;

    // Unknown root elements first (ES-DE writes <alternativeEmulator> before the games)
    for raw in &root_unknowns {
        // Raw bytes bypass the writer's indentation, so start each one on its own line
        writer.get_mut().extend_from_slice(b"\n  ");
        writer.get_mut().extend_from_slice(raw.as_bytes());
    }

    // <provider> if present
    if let Some(ref provider) = gamelist.provider {
        write_provider(&mut writer, provider)?;
    }

    // <folder> elements
    for folder in &gamelist.folders {
        write_folder(&mut writer, folder)?;
    }

    // <game> elements
    for game in &gamelist.games {
        let unknowns = unknown_map.get(&game.path);
        write_game(&mut writer, game, unknowns)?;
    }

    // </gameList>
    writer
        .write_event(Event::End(BytesEnd::new("gameList")))
        .map_err(|e| format!("Write error: {}", e))?;

    let bytes = writer.into_inner();
    let mut xml = String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {}", e))?;
    xml.push('\n');
    Ok(xml)
}

fn write_text_element(writer: &mut Writer<Vec<u8>>, tag: &str, value: &str) -> Result<(), String> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|e| format!("Write error: {}", e))?;
    writer
        .write_event(Event::Text(BytesText::new(value)))
        .map_err(|e| format!("Write error: {}", e))?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

fn write_opt_element(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    value: &Option<String>,
) -> Result<(), String> {
    if let Some(ref v) = value {
        write_text_element(writer, tag, v)?;
    }
    Ok(())
}

fn write_provider(writer: &mut Writer<Vec<u8>>, provider: &Provider) -> Result<(), String> {
    writer
        .write_event(Event::Start(BytesStart::new("provider")))
        .map_err(|e| format!("Write error: {}", e))?;

    write_opt_element(writer, "System", &provider.system)?;
    write_opt_element(writer, "software", &provider.software)?;
    write_opt_element(writer, "database", &provider.database)?;
    write_opt_element(writer, "web", &provider.web)?;

    writer
        .write_event(Event::End(BytesEnd::new("provider")))
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

fn write_folder(writer: &mut Writer<Vec<u8>>, folder: &Folder) -> Result<(), String> {
    writer
        .write_event(Event::Start(BytesStart::new("folder")))
        .map_err(|e| format!("Write error: {}", e))?;

    write_text_element(writer, "path", &folder.path)?;
    write_opt_element(writer, "name", &folder.name)?;
    write_opt_element(writer, "desc", &folder.desc)?;
    write_opt_element(writer, "image", &folder.image)?;
    write_opt_element(writer, "thumbnail", &folder.thumbnail)?;

    writer
        .write_event(Event::End(BytesEnd::new("folder")))
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

fn write_game(
    writer: &mut Writer<Vec<u8>>,
    game: &Game,
    unknowns: Option<&Vec<String>>,
) -> Result<(), String> {
    // Build <game> start tag with attributes
    let mut start = BytesStart::new("game");
    if let Some(ref id) = game.id {
        start.push_attribute(("id", id.as_str()));
    }
    if let Some(ref source) = game.source {
        start.push_attribute(("source", source.as_str()));
    }

    // Re-add unknown attributes from original
    if let Some(extras) = unknowns {
        for extra in extras {
            if let Some(rest) = extra.strip_prefix("@attr:") {
                if let Some((k, v)) = rest.split_once('=') {
                    start.push_attribute((k, v));
                }
            }
        }
    }

    writer
        .write_event(Event::Start(start))
        .map_err(|e| format!("Write error: {}", e))?;

    // Write all known fields
    write_text_element(writer, "path", &game.path)?;
    write_opt_element(writer, "name", &game.name)?;
    write_opt_element(writer, "sortname", &game.sortname)?;
    write_opt_element(writer, "desc", &game.desc)?;
    write_opt_element(writer, "image", &game.image)?;
    write_opt_element(writer, "thumbnail", &game.thumbnail)?;
    write_opt_element(writer, "marquee", &game.marquee)?;
    write_opt_element(writer, "fanart", &game.fanart)?;
    write_opt_element(writer, "titleshot", &game.titleshot)?;
    write_opt_element(writer, "boxback", &game.boxback)?;
    write_opt_element(writer, "boxart", &game.boxart)?;
    write_opt_element(writer, "cartridge", &game.cartridge)?;
    write_opt_element(writer, "wheel", &game.wheel)?;
    write_opt_element(writer, "mix", &game.mix)?;
    write_opt_element(writer, "manual", &game.manual)?;
    write_opt_element(writer, "video", &game.video)?;
    write_opt_element(writer, "rating", &game.rating)?;
    write_opt_element(writer, "releasedate", &game.releasedate)?;
    write_opt_element(writer, "developer", &game.developer)?;
    write_opt_element(writer, "publisher", &game.publisher)?;
    write_opt_element(writer, "genre", &game.genre)?;
    write_opt_element(writer, "genreid", &game.genreid)?;
    write_opt_element(writer, "players", &game.players)?;
    write_opt_element(writer, "family", &game.family)?;
    write_opt_element(writer, "arcadesystemname", &game.arcadesystemname)?;
    write_opt_element(writer, "region", &game.region)?;
    write_opt_element(writer, "lang", &game.lang)?;
    write_opt_element(writer, "favorite", &game.favorite)?;
    write_opt_element(writer, "hidden", &game.hidden)?;
    write_opt_element(writer, "kidgame", &game.kidgame)?;
    write_opt_element(writer, "playcount", &game.playcount)?;
    write_opt_element(writer, "lastplayed", &game.lastplayed)?;
    write_opt_element(writer, "gametime", &game.gametime)?;
    write_opt_element(writer, "emulator", &game.emulator)?;
    write_opt_element(writer, "core", &game.core)?;
    write_opt_element(writer, "hash", &game.hash)?;
    write_opt_element(writer, "md5", &game.md5)?;
    write_opt_element(writer, "crc32", &game.crc32)?;
    write_opt_element(writer, "cheevosHash", &game.cheevos_hash)?;
    write_opt_element(writer, "cheevosId", &game.cheevos_id)?;

    // Write unknown elements preserved from original XML
    if let Some(extras) = unknowns {
        for extra in extras {
            if extra.starts_with("@attr:") {
                continue; // Already handled as attributes
            }
            // Write raw XML directly
            writer
                .get_mut()
                .extend_from_slice(extra.as_bytes());
        }
    }

    writer
        .write_event(Event::End(BytesEnd::new("game")))
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESDE_GAMELIST: &str = r#"<?xml version="1.0"?>
<gameList>
	<alternativeEmulator>
		<label>Snes9x - Current</label>
	</alternativeEmulator>
	<game>
		<path>./Juego Ficticio.sfc</path>
		<name>Juego Ficticio</name>
		<completed>true</completed>
		<altemulator>Mesen &amp; co</altemulator>
	</game>
</gameList>
"#;

    #[test]
    fn preserves_unknown_root_elements() {
        let gamelist = parse_gamelist(ESDE_GAMELIST).unwrap();
        let out = serialize_gamelist(&gamelist, Some(ESDE_GAMELIST)).unwrap();

        assert!(out.contains("\n  <alternativeEmulator>"));
        assert!(out.contains("<label>Snes9x - Current</label>"));
        assert!(out.contains("</alternativeEmulator>"));
        // Root unknowns go before the games, like ES-DE writes them
        assert!(out.find("<alternativeEmulator>").unwrap() < out.find("<game>").unwrap());
    }

    #[test]
    fn preserves_unknown_game_fields_with_escaping() {
        let gamelist = parse_gamelist(ESDE_GAMELIST).unwrap();
        let out = serialize_gamelist(&gamelist, Some(ESDE_GAMELIST)).unwrap();

        assert!(out.contains("<completed>true</completed>"));
        assert!(out.contains("<altemulator>Mesen &amp; co</altemulator>"));
        // The result must still be a valid gamelist
        let reparsed = parse_gamelist(&out).unwrap();
        assert_eq!(reparsed.games.len(), 1);
        assert_eq!(reparsed.games[0].name.as_deref(), Some("Juego Ficticio"));
    }

    #[test]
    fn round_trip_is_stable() {
        let gamelist = parse_gamelist(ESDE_GAMELIST).unwrap();
        let first = serialize_gamelist(&gamelist, Some(ESDE_GAMELIST)).unwrap();
        let second = serialize_gamelist(&parse_gamelist(&first).unwrap(), Some(&first)).unwrap();
        assert_eq!(first, second);
    }
}
