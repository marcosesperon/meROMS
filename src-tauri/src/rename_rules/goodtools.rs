//! Fuente «GoodTools → No-Intro» de Renombrar: traduce los códigos cortos de GoodTools
//! (GoodNES, GoodGen…) al estilo No-Intro, manteniendo la región. `Gradius (J)` → `Gradius (Japan)`,
//! `Paperboy (U) [h1]` → `Paperboy (USA) (Hack)`. Lo que no se reconoce se deja como está.

use std::sync::LazyLock;

use regex::Regex;

/// Cada etiqueta entre paréntesis o corchetes
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([^()]*)\)|\[([^\[\]]*)\]").unwrap());

/// Región de GoodTools (el paréntesis entero) → región de No-Intro
fn region(code: &str) -> Option<&'static str> {
    Some(match code {
        "U" => "USA",
        "J" => "Japan",
        "E" => "Europe",
        "W" | "JUE" => "World",
        "JU" => "Japan, USA",
        "UE" => "USA, Europe",
        "JE" => "Japan, Europe",
        "F" => "France",
        "G" => "Germany",
        "S" => "Spain",
        "I" => "Italy",
        "K" => "Korea",
        "A" => "Australia",
        "B" => "Brazil",
        "C" => "China",
        "H" => "Netherlands",
        "Sw" => "Sweden",
        _ => return None,
    })
}

/// Idioma de una traducción de GoodTools (`Spa`, `Eng`, `Chinese`…) → código de dos letras
fn language(name: &str) -> Option<&'static str> {
    let three: String = name.chars().take(3).collect::<String>().to_lowercase();
    Some(match three.as_str() {
        "eng" => "En",
        "spa" | "esp" => "Es",
        "fre" | "fra" => "Fr",
        "ger" | "deu" => "De",
        "ita" => "It",
        "por" | "bra" => "Pt",
        "chi" => "Zh",
        "rus" => "Ru",
        "dut" | "nld" => "Nl",
        "swe" => "Sv",
        "kor" => "Ko",
        "jap" | "jpn" => "Ja",
        "pol" => "Pl",
        "gre" => "El",
        _ => return None,
    })
}

/// Qué es cada etiqueta, para ordenarlas como No-Intro: región, revisión y luego lo demás
enum Tag {
    Region(String),
    Revision(String),
    Other(String),
    Drop,
}

/// Una etiqueta de paréntesis (`inner` sin los paréntesis)
fn paren(inner: &str) -> Tag {
    if let Some(r) = region(inner) {
        return Tag::Region(format!("({})", r));
    }
    // «PRG1» (revisión de NES) → «Rev 1»; «V1.1» → «v1.1»
    if let Some(n) = inner.strip_prefix("PRG").filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
        return Tag::Revision(format!("(Rev {})", n));
    }
    if let Some(v) = inner.strip_prefix(['V', 'v']).filter(|v| v.starts_with(|c: char| c.is_ascii_digit()) && v.chars().all(|c| c.is_ascii_digit() || c == '.')) {
        return Tag::Revision(format!("(v{})", v));
    }
    if inner.starts_with("Rev ") {
        return Tag::Revision(format!("({})", inner));
    }
    // Región ya escrita a lo No-Intro: se queda en su sitio de región
    let no_intro = ["USA", "Japan", "Europe", "World", "Spain", "France", "Germany", "Italy", "Korea", "Australia", "Brazil", "China", "Asia"];
    if inner.split(", ").all(|p| no_intro.contains(&p)) {
        return Tag::Region(format!("({})", inner));
    }
    Tag::Other(format!("({})", inner))
}

/// Una etiqueta de corchetes (`inner` sin los corchetes)
fn bracket(inner: &str) -> Tag {
    // Letra de estado seguida de un número o de nada: [h1], [hM04], [p2], [b]
    let starts = |letter: char| inner.starts_with(letter) && inner[1..].chars().next().is_none_or(|c| c.is_ascii_digit() || c.is_ascii_uppercase() || c == '+');
    if inner == "!" {
        Tag::Drop
    } else if starts('h') {
        Tag::Other("(Hack)".into())
    } else if starts('p') {
        Tag::Other("(Pirate)".into())
    } else if let Some(rest) = inner.strip_prefix("T+").or_else(|| inner.strip_prefix("T-")) {
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        match language(&name) {
            Some(code) => Tag::Other(format!("(Translated {})", code)),
            None => Tag::Other("(Translated)".into()),
        }
    } else {
        Tag::Other(format!("[{}]", inner))
    }
}

/// ¿Es un código propio de GoodTools? (si no hay ninguno, el nombre no se toca, ni se reordena)
fn is_goodtools(c: &regex::Captures) -> bool {
    match (c.get(1), c.get(2)) {
        // Región de una o dos letras, «PRG1» o «V1.1» (No-Intro escribe «Rev 1» y «v1.1»)
        (Some(p), _) => {
            let p = p.as_str();
            region(p).is_some() || (matches!(paren(p), Tag::Revision(_)) && (p.starts_with("PRG") || p.starts_with('V')))
        }
        (_, Some(b)) => !matches!(bracket(b.as_str()), Tag::Other(ref o) if o.starts_with('[')),
        _ => false,
    }
}

/// El nombre (sin extensión) con los códigos de GoodTools traducidos a No-Intro. Si no hay
/// nada que traducir, sale igual.
pub fn to_no_intro(stem: &str) -> String {
    let Some(first) = TAG.find(stem) else { return stem.to_string() };
    if !TAG.captures_iter(stem).any(|c| is_goodtools(&c)) {
        return stem.to_string();
    }
    // Texto suelto entre o tras las etiquetas: no se reordena nada, solo se traduce en su sitio
    let loose = TAG.replace_all(&stem[first.start()..], "").trim().to_string();
    let tags: Vec<Tag> = TAG
        .captures_iter(stem)
        .map(|c| match (c.get(1), c.get(2)) {
            (Some(p), _) => paren(p.as_str()),
            (_, Some(b)) => bracket(b.as_str()),
            _ => Tag::Drop,
        })
        .collect();

    let text = |t: &Tag| match t {
        Tag::Region(s) | Tag::Revision(s) | Tag::Other(s) => Some(s.clone()),
        Tag::Drop => None,
    };
    let mut parts: Vec<String> = Vec::new();
    if loose.is_empty() {
        // Orden de No-Intro: región, revisión, lo demás (sin repetir «(Hack)» y compañía)
        let base = stem[..first.start()].trim_end();
        parts.push(base.to_string());
        for kind in 0..3 {
            for t in &tags {
                let wanted = matches!((kind, t), (0, Tag::Region(_)) | (1, Tag::Revision(_)) | (2, Tag::Other(_)));
                if let Some(s) = text(t).filter(|s| wanted && !parts.contains(s)) {
                    parts.push(s);
                }
            }
        }
        return parts.join(" ");
    }
    let mut i = 0;
    TAG.replace_all(stem, |_: &regex::Captures| {
        let out = text(&tags[i]).unwrap_or_default();
        i += 1;
        out
    })
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goodtools_codes_become_no_intro() {
        for (from, to) in [
            ("Gradius (J)", "Gradius (Japan)"),
            ("Paperboy (U) [h1]", "Paperboy (USA) (Hack)"),
            ("Power Blade (PC10) [!]", "Power Blade (PC10)"),
            ("Chip 'n Dale Rescue Rangers (E) [T+Fre100%_Terminus]", "Chip 'n Dale Rescue Rangers (Europe) (Translated Fr)"),
            ("Saint Seiya (J) [T-Chinese]", "Saint Seiya (Japan) (Translated Zh)"),
            ("Kung-Fu (JU) (PRG1)", "Kung-Fu (Japan, USA) (Rev 1)"),
            ("Zelda (U) [b1]", "Zelda (USA) [b1]"),
            ("Mario (J) [p2] [h1]", "Mario (Japan) (Pirate) (Hack)"),
            ("Robocop Vs The Terminator (Prototype) (U)", "Robocop Vs The Terminator (USA) (Prototype)"),
            ("Super Castlevania 2 (V0.9) (Hack)", "Super Castlevania 2 (v0.9) (Hack)"),
        ] {
            assert_eq!(to_no_intro(from), to, "{}", from);
        }
    }

    #[test]
    fn what_is_not_goodtools_is_left_alone() {
        for name in [
            "Sonic (USA)",
            "Sonic (USA, Europe) (Rev 1)",
            "Ninja Ryuukenden 3 (c)",
            "0010 Circus [Chinese version]",
            "Super Mario Bros 2 (Kaiser Pirate)",
            "Donkey Kong",
            "Game (Beta) (USA)",
        ] {
            assert_eq!(to_no_intro(name), name);
        }
    }
}
