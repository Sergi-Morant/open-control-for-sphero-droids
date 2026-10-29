import Foundation

struct LightChannel: Identifiable {
    enum Style {
        case rgb(UInt16, UInt16, UInt16)   // máscaras v2 de rojo, verde y azul
        case mono(UInt16)                  // máscara v2 de un solo LED
        case v1Main                        // LED RGB principal de BB-8
        case v1Back                        // LED trasero de BB-8
    }
    let title: String
    let style: Style
    var id: String { title }
}

/// Tipos de droide soportados por la app original (ToyTypes) y su prefijo de nombre BLE (ToyPrefix).
enum DroidKind: String, CaseIterable, Identifiable {
    case bb8, r2d2, bb9e, r2q5
    var id: String { rawValue }

    var prefix: String { ["bb8": "BB", "r2d2": "D2", "bb9e": "GB", "r2q5": "Q5"][rawValue]! }
    var title: String { ["bb8": "BB-8", "r2d2": "R2-D2", "bb9e": "BB-9E", "r2q5": "R2-Q5"][rawValue]! }
    var symbol: String { self == .bb8 || self == .bb9e ? "circle.circle" : "cylinder" }

    static func from(bleName: String) -> DroidKind? {
        allCases.first { bleName.hasPrefix($0.prefix + "-") }
    }

    var usesV2: Bool { self != .bb8 }
    var hasAudio: Bool { self != .bb8 }
    var hasDome: Bool { self == .r2d2 || self == .r2q5 }
    var hasLegs: Bool { hasDome }

    var sounds: [DroidSound] {
        switch self {
        case .bb8: return []
        case .bb9e: return Catalog.bb9eSounds
        case .r2d2, .r2q5: return Catalog.r2d2Sounds   // el R2-Q5 comparte la lista del R2-D2
        }
    }

    var animations: [DroidAnimation] {
        switch self {
        case .bb8: return []
        case .r2d2: return Catalog.r2d2Animations
        case .r2q5: return Catalog.r2q5Animations
        case .bb9e: return Catalog.bb9eAnimations
        }
    }

    var lights: [LightChannel] {
        switch self {
        case .r2d2, .r2q5:
            return [LightChannel(title: "LED frontal", style: .rgb(0x01, 0x02, 0x04)),
                    LightChannel(title: "LED trasero", style: .rgb(0x10, 0x20, 0x40)),
                    LightChannel(title: "Pantalla lógica", style: .mono(0x08)),
                    LightChannel(title: "Holoproyector", style: .mono(0x80))]
        case .bb9e:
            return [LightChannel(title: "Cuerpo", style: .rgb(0x01, 0x02, 0x04)),
                    LightChannel(title: "Luz de apuntado", style: .mono(0x08)),
                    LightChannel(title: "Luz de la cabeza", style: .mono(0x10))]
        case .bb8:
            return [LightChannel(title: "LED principal", style: .v1Main),
                    LightChannel(title: "LED trasero (apuntado)", style: .v1Back)]
        }
    }
}

extension DroidAnimation {
    var categoryTitle: String {
        switch category {
        case "Emote": return "Emociones"
        case "Idle": return "Reposo"
        case "Patrol": return "Patrulla"
        case "WWM": return "Reacciones (Watch With Me)"
        case "Reaction": return "Reacciones"
        default: return category
        }
    }
}

extension DroidSound {
    /// Grupo del sonido a partir de su nombre ("R2_Chatty_12" → "Chatty").
    var group: String {
        var n = name.replacingOccurrences(of: "Play_BB9E_", with: "").replacingOccurrences(of: "R2_", with: "")
            .replacingOccurrences(of: "R2 ", with: "").replacingOccurrences(of: "_", with: " ")
        while let last = n.last, last.isNumber || last == " " { n.removeLast() }
        return n.isEmpty ? name : n
    }
}
