//! Traducir descripciones con DeepL (clave del usuario en Ajustes; el plan gratuito sirve).

use serde::Serialize;
use serde_json::Value;

/// DeepL admite hasta 50 textos por petición
const BATCH: usize = 50;

/// Las claves del plan gratuito acaban en «:fx» y van a otro servidor
fn api_url(key: &str) -> &'static str {
    if key.trim().ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

/// Código de DeepL del idioma de la app
fn target_code(lang: &str) -> &'static str {
    match lang {
        "es" => "ES",
        _ => "EN-US",
    }
}

/// Lo traducido de un lote: None si DeepL lo detecta ya en el idioma de destino
fn parse_response(body: &Value, target: &str) -> Result<Vec<Option<String>>, String> {
    let list = body["translations"].as_array().ok_or("DeepL: unexpected response")?;
    let base = target.split('-').next().unwrap_or(target);
    Ok(list
        .iter()
        .map(|t| {
            let source = t["detected_source_language"].as_str().unwrap_or("");
            if source.eq_ignore_ascii_case(base) {
                None
            } else {
                t["text"].as_str().map(String::from)
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    /// None = ya estaba en ese idioma (se deja como está)
    pub text: Option<String>,
}

/// Traduce textos al idioma `target` («es» | «en»), en lotes. Mismo orden que `texts`.
#[tauri::command]
pub async fn translate_texts(app: tauri::AppHandle, texts: Vec<String>, target: String) -> Result<Vec<Translation>, String> {
    let settings = super::settings::get_settings(app)?;
    let key = settings.deepl_apikey.filter(|k| !k.trim().is_empty()).ok_or("DeepL API key not set (Settings > General > Translation)")?;
    let code = target_code(&target);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(BATCH) {
        let response = client
            .post(api_url(&key))
            .header("Authorization", format!("DeepL-Auth-Key {}", key.trim()))
            .json(&serde_json::json!({ "text": chunk, "target_lang": code }))
            .send()
            .await
            .map_err(|e| format!("DeepL request failed: {}", e))?;
        let status = response.status();
        if !status.is_success() {
            // 403 = clave mala; 456 = cuota del mes agotada
            let why = match status.as_u16() {
                403 => "invalid API key".to_string(),
                456 => "monthly quota exceeded".to_string(),
                _ => response.text().await.unwrap_or_default().chars().take(200).collect(),
            };
            return Err(format!("DeepL error: HTTP {} ({})", status.as_u16(), why));
        }
        let body: Value = response.json().await.map_err(|e| format!("DeepL: {}", e))?;
        let translated = parse_response(&body, code)?;
        if translated.len() != chunk.len() {
            return Err("DeepL: unexpected number of translations".into());
        }
        out.extend(translated.into_iter().map(|text| Translation { text }));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_keys_use_the_free_server() {
        assert_eq!(api_url("abc-123:fx"), "https://api-free.deepl.com/v2/translate");
        assert_eq!(api_url("abc-123"), "https://api.deepl.com/v2/translate");
        assert_eq!(target_code("es"), "ES");
        assert_eq!(target_code("en"), "EN-US");
    }

    #[test]
    fn texts_already_in_the_target_language_are_left_alone() {
        let body = serde_json::json!({"translations": [
            {"detected_source_language": "EN", "text": "Un fontanero salta."},
            {"detected_source_language": "ES", "text": "Ya estaba en español."}
        ]});
        assert_eq!(parse_response(&body, "ES").unwrap(), vec![Some("Un fontanero salta.".to_string()), None]);
        let en = serde_json::json!({"translations": [{"detected_source_language": "EN", "text": "x"}]});
        assert_eq!(parse_response(&en, "EN-US").unwrap(), vec![None]);
    }
}
