//! El título que se propone al renombrar, sacado del nombre de fichero. Es la misma lógica que
//! `src/utils/renameSources.ts` (`titleFromName`, `proposedTitle`) y `src/utils/articles.ts`: la
//! ventana la sigue usando para lo que se escribe a mano, y aquí se usa para la propuesta, que
//! se recalcula hasta que deja de cambiar (ver `commands/rename.rs` `propose`). Si se cambia una,
//! hay que cambiar la otra.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Dónde va el artículo del título: como venga, al final («Zelda, The») o al principio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleMode {
    #[default]
    Keep,
    End,
    Front,
}

const ARTICLES: [&str; 7] = ["The", "A", "An", "El", "La", "Los", "Las"];

/// «Nombre (USA) [!]» → («Nombre», « (USA) [!]»)
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*?)(\s*[(\[].*)$").unwrap());
/// El nombre sin etiquetas (lo de antes del primer paréntesis o corchete)
static BEFORE_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*?\S.*?)\s*[(\[].*$").unwrap());

fn is_article(word: &str) -> bool {
    ARTICLES.iter().any(|a| a.eq_ignore_ascii_case(word))
}

fn split_tags(name: &str) -> (String, String) {
    match TAGS.captures(name) {
        Some(c) if !c[1].trim().is_empty() => (c[1].trim_end().to_string(), c[2].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

/// «The Legend of Zelda (USA)» → «Legend of Zelda, The (USA)»; si no empieza por artículo, None
pub fn article_to_end(name: &str) -> Option<String> {
    let (title, tags) = split_tags(name.trim());
    let space = title.find(' ').filter(|&i| i > 0)?;
    let first = &title[..space];
    let rest = title[space + 1..].trim();
    (is_article(first) && !rest.is_empty()).then(|| format!("{rest}, {first}{tags}"))
}

/// «Legend of Zelda, The (USA)» → «The Legend of Zelda (USA)»; si no acaba en «, artículo», None
pub fn article_to_front(name: &str) -> Option<String> {
    let (title, tags) = split_tags(name.trim());
    let comma = title.rfind(", ").filter(|&i| i > 0)?;
    let last = title[comma + 2..].trim();
    is_article(last).then(|| format!("{last} {}{tags}", title[..comma].trim()))
}

fn move_article(name: &str, article: ArticleMode) -> Option<String> {
    match article {
        ArticleMode::End => article_to_end(name),
        ArticleMode::Front => article_to_front(name),
        ArticleMode::Keep => None,
    }
}

/// El título que sale de un nombre de fichero: sin etiquetas y con el artículo donde toque
pub fn title_from_name(name: &str, article: ArticleMode) -> String {
    let title = BEFORE_TAGS.captures(name).map_or(name, |c| c.get(1).map_or(name, |m| m.as_str())).trim().to_string();
    move_article(&title, article).unwrap_or(title)
}

/// Para comparar títulos sin mirar mayúsculas, acentos ni signos («Contra III: The…» =
/// «Contra III - The…»). La ventana quita los acentos con `normalize("NFD")`; aquí, con las
/// letras latinas acentuadas de uso normal.
fn comparable(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(fold_accent)
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        c => c,
    }
}

/// Título propuesto: el sacado del nombre de fichero, salvo que diga lo mismo que el actual
/// (solo cambian signos o mayúsculas: el actual suele venir del scraper y está mejor escrito).
/// Con el artículo cambiado de sitio sí se propone. Si ninguna fuente dio nombre (`from_source`
/// false), el título solo se cambia si era una copia del nombre de fichero de antes.
pub fn proposed_title(file_name: &str, current: Option<&str>, article: ArticleMode, from_source: bool, old_file_name: &str) -> String {
    let from_file = title_from_name(file_name, article);
    let Some(current) = current.filter(|c| !c.is_empty()) else { return from_file };
    let base = move_article(current, article).unwrap_or_else(|| current.to_string());
    let copied_from_file = comparable(&title_from_name(current, ArticleMode::Keep)) == comparable(&title_from_name(old_file_name, ArticleMode::Keep));
    if !from_source && !copied_from_file {
        return base;
    }
    if comparable(&base) == comparable(&from_file) {
        base
    } else {
        from_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articulos() {
        assert_eq!(article_to_end("The Legend of Zelda (USA)").as_deref(), Some("Legend of Zelda, The (USA)"));
        assert_eq!(article_to_end("Legend of Zelda"), None);
        assert_eq!(article_to_end("The"), None);
        assert_eq!(article_to_front("Legend of Zelda, The (USA) [!]").as_deref(), Some("The Legend of Zelda (USA) [!]"));
        assert_eq!(article_to_front("Zelda, Link"), None);
    }

    #[test]
    fn titulo_desde_el_fichero() {
        assert_eq!(title_from_name("The Legend of Kage (USA) (En)", ArticleMode::End), "Legend of Kage, The");
        assert_eq!(title_from_name("Sonic (USA)", ArticleMode::Keep), "Sonic");
        assert_eq!(title_from_name("(Prototype)", ArticleMode::Keep), "(Prototype)");
    }

    #[test]
    fn titulo_propuesto() {
        // Solo signos: se queda el actual (mejor escrito)
        assert_eq!(proposed_title("Contra III - The Alien Wars (USA)", Some("Contra III: The Alien Wars"), ArticleMode::Keep, true, ""), "Contra III: The Alien Wars");
        assert_eq!(proposed_title("Pokemon Crazy Drummer (En)", Some("Pokémon Crazy Drummer"), ArticleMode::Keep, true, ""), "Pokémon Crazy Drummer");
        // El artículo, al final
        assert_eq!(proposed_title("The Firemen (En)", Some("The Firemen"), ArticleMode::End, true, ""), "Firemen, The");
        // Distinto: el del fichero
        assert_eq!(proposed_title("Dune - The Battle for Arrakis (Europe)", Some("Dune II : The Battle for Arrakis"), ArticleMode::Keep, true, ""), "Dune - The Battle for Arrakis");
        // Sin fuente: solo si el título era una copia del nombre de fichero de antes
        assert_eq!(proposed_title("Green Corps", Some("0004 Green Corps"), ArticleMode::Keep, false, "0004 Green Corps"), "Green Corps");
        assert_eq!(proposed_title("Green Corps", Some("Green Corps Deluxe"), ArticleMode::Keep, false, "0004 Green Corps"), "Green Corps Deluxe");
        // Sin título: el del fichero
        assert_eq!(proposed_title("Sonic (USA)", None, ArticleMode::Keep, true, ""), "Sonic");
    }
}
