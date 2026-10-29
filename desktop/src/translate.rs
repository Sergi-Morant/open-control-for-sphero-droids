//! Traduce al español los nombres originales de sonidos y animaciones.
//! El nombre original se conserva y se muestra entre paréntesis.

/// Palabras que no aportan nada: prefijos del droide y la categoría (ya aparece como sección).
const DROPPED: [&str; 9] = ["r2", "r2d2", "r2q5", "bb9e", "play", "emote", "wwm", "wmm", "patrol"];

fn word(key: &str) -> Option<&'static str> {
    Some(match key {
        // frases de varias palabras
        "access panels" => "Acceso a paneles",
        "engage hyperdrive" => "Activar hiperimpulso",
        "head spin" | "head turn" => "Giro de cabeza",
        "patol patroling" => "Patrullando",
        // sonidos
        "alarm" => "Alarma",
        "chatty" => "Parlanchín",
        "excited" => "Emocionado",
        "hey" => "¡Eh!",
        "negative" => "Negativo",
        "positive" => "Positivo",
        "sad" => "Triste",
        "fall" => "Caída",
        "hit" => "Golpe",
        "step" => "Paso",
        "annoyed" => "Molesto",
        "burnout" => "Quemado",
        "laugh" => "Risa",
        "motor" => "Motor",
        "scream" => "Grito",
        "shortout" | "shortcircut" => "Cortocircuito",
        "extra" => "Extra",
        // animaciones
        "charger" => "Cargador",
        "angry" => "Enfadado",
        "drive" => "Conducir",
        "happy" => "Feliz",
        "ionblast" => "Ion Blast",
        "no" => "No",
        "sassy" => "Descarado",
        "scared" => "Asustado",
        "spin" => "Giro",
        "yes" => "Sí",
        "scan" => "Escaneo",
        "sleep" => "Dormir",
        "surprised" => "Sorprendido",
        "idle" => "Reposo",
        "patroling" => "Patrullando",
        "anxious" => "Ansioso",
        "bow" => "Reverencia",
        "concern" => "Preocupación",
        "curious" => "Curioso",
        "doubletake" => "Doble mirada",
        "fiery" => "Ardiente",
        "frustrated" => "Frustrado",
        "jittery" => "Inquieto",
        "longshake" => "Sacudida larga",
        "ominous" => "Siniestro",
        "relieved" => "Aliviado",
        "shake" => "Sacudida",
        "taunting" => "Burlón",
        "whisper" => "Susurro",
        "yelling" => "Gritando",
        "yoohoo" => "¡Yuju!",
        "scansweep" => "Barrido de escaneo",
        "affirmative" => "Afirmativo",
        "agitated" => "Agitado",
        "content" => "Contento",
        "greetings" => "Saludos",
        "nervous" => "Nervioso",
        "retreat" => "Retirada",
        "understood" => "Entendido",
        "attention" => "Atención",
        "search" => "Búsqueda",
        _ => return None,
    })
}

/// "R2_Access Panels" → "Acceso a paneles"; "R2D2_WWM_Angry" → "Enfadado"; "R2_Chatty_12" → "Parlanchín 12".
pub fn title(raw: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();
    for t in raw.replace('_', " ").split_whitespace() {
        if DROPPED.contains(&t.to_lowercase().as_str()) {
            continue;
        }
        // "Charger1" → "Charger", "1"
        let digits = t.chars().rev().take_while(|c| c.is_ascii_digit()).count();
        let split = t.len() - digits;
        if digits > 0 && split > 0 && t[..split].chars().any(|c| c.is_alphabetic()) {
            tokens.push(t[..split].to_string());
            tokens.push(t[split..].to_string());
        } else {
            tokens.push(t.to_string());
        }
    }
    if tokens.is_empty() {
        return raw.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if i + 1 < tokens.len() {
            if let Some(w) = word(&format!("{} {}", tokens[i], tokens[i + 1]).to_lowercase()) {
                out.push(w.to_string());
                i += 2;
                continue;
            }
        }
        out.push(word(&tokens[i].to_lowercase()).map(str::to_string).unwrap_or_else(|| tokens[i].clone()));
        i += 1;
    }
    out.join(" ")
}

/// "Acceso a paneles (R2_Access Panels)"
pub fn title_with_original(raw: &str) -> String {
    format!("{} ({})", title(raw), raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_the_requested_example() {
        assert_eq!(title_with_original("R2_Access Panels"), "Acceso a paneles (R2_Access Panels)");
    }

    #[test]
    fn keeps_numbers_and_drops_prefixes() {
        assert_eq!(title("R2_Chatty_12"), "Parlanchín 12");
        assert_eq!(title("R2 Hit_1"), "Golpe 1");
        assert_eq!(title("R2D2_Charger1"), "Cargador 1");
        assert_eq!(title("R2D2_WWM_DoubleTake"), "Doble mirada");
        assert_eq!(title("R2D2_Patol_Patroling"), "Patrullando");
        assert_eq!(title("Play_BB9E_Head_Turn_1"), "Giro de cabeza 1");
    }

    #[test]
    fn every_catalog_name_is_translated() {
        use crate::kind::Kind;
        for k in Kind::ALL {
            let names = k.sounds().iter().map(|s| s.name).chain(k.animations().iter().map(|a| a.name));
            for raw in names {
                let t = title(raw);
                let raw_words: Vec<String> = raw.replace('_', " ").split_whitespace().map(|w| w.to_lowercase()).collect();
                for w in t.split_whitespace() {
                    let lw = w.to_lowercase();
                    let allowed = ["extra", "motor", "no", "ion", "blast"];
                    assert!(
                        !(raw_words.contains(&lw) && w.chars().next().unwrap().is_alphabetic() && !allowed.contains(&lw.as_str())),
                        "sin traducir: {w} en {raw}"
                    );
                }
            }
        }
    }
}
