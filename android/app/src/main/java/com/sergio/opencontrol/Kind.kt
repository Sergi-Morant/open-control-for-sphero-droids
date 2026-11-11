package com.sergio.opencontrol

data class Sound(val name: String, val sector: Int) {
    /// Nombre en español con el original entre paréntesis.
    val title: String get() = Spanish.titleWithOriginal(name)
    val groupTitle: String get() = Spanish.title(group)

    /// Grupo del sonido a partir de su nombre ("R2_Chatty_12" → "Chatty").
    val group: String by lazy {
        val n = name.replace("Play_BB9E_", "").replace("R2_", "").replace("R2 ", "").replace("_", " ")
            .trimEnd { it.isDigit() || it == ' ' }
        n.ifEmpty { name }
    }
}

data class Animation(val name: String, val index: Int, val category: String, val seconds: Float) {
    val title: String get() = Spanish.titleWithOriginal(name)
    val isIonBlast: Boolean get() = name.contains("IonBlast", ignoreCase = true)
    val categoryTitle: String
        get() = when (category) {
            "Emote" -> "Emociones"
            "Idle" -> "Reposo"
            "Patrol" -> "Patrulla"
            "WWM" -> "Reacciones (Watch With Me)"
            "Reaction" -> "Reacciones"
            else -> category
        }
}

sealed interface LightStyle {
    /// Máscaras v2 de rojo, verde y azul.
    data class Rgb(val r: Int, val g: Int, val b: Int) : LightStyle
    /// Máscara v2 de un solo LED.
    data class Mono(val mask: Int) : LightStyle
    /// LED RGB principal de BB-8.
    data object V1Main : LightStyle
    /// LED trasero de BB-8.
    data object V1Back : LightStyle
}

data class LightChannel(val title: String, val style: LightStyle)

/// Tipos de droide soportados por la app original y su prefijo de nombre BLE.
enum class Kind(val prefix: String, val title: String) {
    BB8("BB", "BB-8"), R2D2("D2", "R2-D2"), BB9E("GB", "BB-9E"), R2Q5("Q5", "R2-Q5");

    val usesV2: Boolean get() = this != BB8
    val hasAudio: Boolean get() = this != BB8
    val hasDome: Boolean get() = this == R2D2 || this == R2Q5

    val sounds: List<Sound>
        get() = when (this) {
            BB8 -> emptyList()
            BB9E -> Catalog.bb9eSounds
            R2D2, R2Q5 -> Catalog.r2d2Sounds // el R2-Q5 comparte la lista del R2-D2
        }

    val animations: List<Animation>
        get() = when (this) {
            BB8 -> emptyList()
            R2D2 -> Catalog.r2d2Animations
            R2Q5 -> Catalog.r2q5Animations
            BB9E -> Catalog.bb9eAnimations
        }

    val lights: List<LightChannel>
        get() = when (this) {
            R2D2, R2Q5 -> listOf(
                LightChannel("LED frontal", LightStyle.Rgb(0x01, 0x02, 0x04)),
                LightChannel("LED trasero", LightStyle.Rgb(0x10, 0x20, 0x40)),
                LightChannel("Pantalla lógica", LightStyle.Mono(0x08)),
                LightChannel("Holoproyector", LightStyle.Mono(0x80)),
            )
            BB9E -> listOf(
                LightChannel("Cuerpo", LightStyle.Rgb(0x01, 0x02, 0x04)),
                LightChannel("Luz de apuntado", LightStyle.Mono(0x08)),
                LightChannel("Luz de la cabeza", LightStyle.Mono(0x10)),
            )
            BB8 -> listOf(
                LightChannel("LED principal", LightStyle.V1Main),
                LightChannel("LED trasero (apuntado)", LightStyle.V1Back),
            )
        }

    /// Versiones de firmware incluidas en el APK original.
    val bundledFirmware: String
        get() = when (this) {
            BB8 -> "ST 4.69"
            R2D2 -> "Nordic 7.0.101 · ST 7.1.86"
            BB9E -> "Nordic 7.0.68 · ST 7.0.62"
            R2Q5 -> "Nordic 7.0.102 · ST 7.1.87"
        }

    companion object {
        fun fromBleName(name: String): Kind? = entries.firstOrNull { name.startsWith(it.prefix + "-") }
    }
}

/// Traduce al español los nombres originales de sonidos y animaciones.
object Spanish {
    /// Palabras que no aportan nada: prefijos del droide y la categoría (ya aparece como sección).
    private val dropped = setOf("r2", "r2d2", "r2q5", "bb9e", "play", "emote", "wwm", "wmm", "patrol")

    private val words = mapOf(
        // frases de varias palabras
        "access panels" to "Acceso a paneles", "engage hyperdrive" to "Activar hiperimpulso",
        "head spin" to "Giro de cabeza", "head turn" to "Giro de cabeza", "patol patroling" to "Patrullando",
        // sonidos
        "alarm" to "Alarma", "chatty" to "Parlanchín", "excited" to "Emocionado", "hey" to "¡Eh!", "negative" to "Negativo",
        "positive" to "Positivo", "sad" to "Triste", "fall" to "Caída", "hit" to "Golpe", "step" to "Paso",
        "annoyed" to "Molesto", "burnout" to "Quemado", "laugh" to "Risa", "motor" to "Motor", "scream" to "Grito",
        "shortout" to "Cortocircuito", "extra" to "Extra",
        // animaciones
        "charger" to "Cargador", "angry" to "Enfadado", "drive" to "Conducir", "happy" to "Feliz", "ionblast" to "Ion Blast",
        "no" to "No", "sassy" to "Descarado", "scared" to "Asustado", "spin" to "Giro", "yes" to "Sí", "scan" to "Escaneo",
        "sleep" to "Dormir", "surprised" to "Sorprendido", "idle" to "Reposo", "patroling" to "Patrullando",
        "anxious" to "Ansioso", "bow" to "Reverencia", "concern" to "Preocupación", "curious" to "Curioso",
        "doubletake" to "Doble mirada", "fiery" to "Ardiente", "frustrated" to "Frustrado", "jittery" to "Inquieto",
        "longshake" to "Sacudida larga", "ominous" to "Siniestro", "relieved" to "Aliviado", "shake" to "Sacudida",
        "taunting" to "Burlón", "whisper" to "Susurro", "yelling" to "Gritando", "yoohoo" to "¡Yuju!",
        "scansweep" to "Barrido de escaneo", "affirmative" to "Afirmativo", "agitated" to "Agitado", "content" to "Contento",
        "greetings" to "Saludos", "nervous" to "Nervioso", "retreat" to "Retirada", "understood" to "Entendido",
        "attention" to "Atención", "search" to "Búsqueda", "shortcircut" to "Cortocircuito",
    )

    private val trailingDigits = Regex("^(.*[A-Za-z].*?)(\\d+)$")

    /// "R2_Access Panels" → "Acceso a paneles"; "R2D2_WWM_Angry" → "Enfadado"; "R2_Chatty_12" → "Parlanchín 12".
    fun title(raw: String): String {
        val tokens = mutableListOf<String>()
        for (t in raw.replace("_", " ").split(" ").filter { it.isNotEmpty() }) {
            if (t.lowercase() in dropped) continue
            // "Charger1" → "Charger", "1"
            val m = trailingDigits.matchEntire(t)
            if (m != null) {
                tokens.add(m.groupValues[1])
                tokens.add(m.groupValues[2])
            } else {
                tokens.add(t)
            }
        }
        if (tokens.isEmpty()) return raw
        val out = mutableListOf<String>()
        var i = 0
        while (i < tokens.size) {
            val pair = if (i + 1 < tokens.size) words["${tokens[i]} ${tokens[i + 1]}".lowercase()] else null
            if (pair != null) {
                out.add(pair)
                i += 2
            } else {
                out.add(words[tokens[i].lowercase()] ?: tokens[i])
                i += 1
            }
        }
        return out.joinToString(" ")
    }

    /// "Acceso a paneles (R2_Access Panels)"
    fun titleWithOriginal(raw: String): String = "${title(raw)} (${raw})"
}
