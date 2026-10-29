import Foundation

/// Traduce al español los nombres originales de sonidos y animaciones.
/// El nombre original se conserva y se muestra entre paréntesis.
enum Spanish {
    /// Palabras que no aportan nada: prefijos del droide y la categoría (ya aparece como sección).
    private static let dropped: Set<String> = ["r2", "r2d2", "r2q5", "bb9e", "play", "emote", "wwm", "wmm", "patrol"]

    private static let words: [String: String] = [
        // frases de varias palabras
        "access panels": "Acceso a paneles", "engage hyperdrive": "Activar hiperimpulso",
        "head spin": "Giro de cabeza", "head turn": "Giro de cabeza", "patol patroling": "Patrullando",
        // sonidos
        "alarm": "Alarma", "chatty": "Parlanchín", "excited": "Emocionado", "hey": "¡Eh!", "negative": "Negativo",
        "positive": "Positivo", "sad": "Triste", "fall": "Caída", "hit": "Golpe", "step": "Paso",
        "annoyed": "Molesto", "burnout": "Quemado", "laugh": "Risa", "motor": "Motor", "scream": "Grito",
        "shortout": "Cortocircuito", "extra": "Extra",
        // animaciones
        "charger": "Cargador", "angry": "Enfadado", "drive": "Conducir", "happy": "Feliz", "ionblast": "Ion Blast",
        "no": "No", "sassy": "Descarado", "scared": "Asustado", "spin": "Giro", "yes": "Sí", "scan": "Escaneo",
        "sleep": "Dormir", "surprised": "Sorprendido", "idle": "Reposo", "patroling": "Patrullando",
        "anxious": "Ansioso", "bow": "Reverencia", "concern": "Preocupación", "curious": "Curioso",
        "doubletake": "Doble mirada", "fiery": "Ardiente", "frustrated": "Frustrado", "jittery": "Inquieto",
        "longshake": "Sacudida larga", "ominous": "Siniestro", "relieved": "Aliviado", "shake": "Sacudida",
        "taunting": "Burlón", "whisper": "Susurro", "yelling": "Gritando", "yoohoo": "¡Yuju!",
        "scansweep": "Barrido de escaneo", "affirmative": "Afirmativo", "agitated": "Agitado", "content": "Contento",
        "greetings": "Saludos", "nervous": "Nervioso", "retreat": "Retirada", "understood": "Entendido",
        "attention": "Atención", "search": "Búsqueda", "shortcircut": "Cortocircuito",
    ]

    /// "R2_Access Panels" → "Acceso a paneles"; "R2D2_WWM_Angry" → "Enfadado"; "R2_Chatty_12" → "Parlanchín 12".
    static func title(_ raw: String) -> String {
        var tokens: [String] = []
        for t in raw.replacingOccurrences(of: "_", with: " ").split(separator: " ").map(String.init) {
            if dropped.contains(t.lowercased()) { continue }
            // "Charger1" → "Charger", "1"
            if let r = t.range(of: "\\d+$", options: .regularExpression), r.lowerBound != t.startIndex,
               t[..<r.lowerBound].contains(where: \.isLetter) {
                tokens += [String(t[..<r.lowerBound]), String(t[r])]
            } else {
                tokens.append(t)
            }
        }
        guard !tokens.isEmpty else { return raw }
        var out: [String] = []
        var i = 0
        while i < tokens.count {
            if i + 1 < tokens.count, let w = words["\(tokens[i]) \(tokens[i + 1])".lowercased()] {
                out.append(w); i += 2
            } else {
                out.append(words[tokens[i].lowercased()] ?? tokens[i]); i += 1
            }
        }
        return out.joined(separator: " ")
    }

    /// "Acceso a paneles (R2_Access Panels)"
    static func titleWithOriginal(_ raw: String) -> String { "\(title(raw)) (\(raw))" }
}
