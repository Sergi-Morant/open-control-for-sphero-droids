import GameController
import SwiftUI

/// Botones asignables. Se identifican por posición, así que PlayStation sigue la misma estructura que Xbox:
/// Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT.
enum PadButton: String, CaseIterable, Identifiable {
    case a, b, x, y, leftShoulder, rightShoulder, dpadUp, dpadDown, dpadLeft, dpadRight, menu, leftStick, rightStick
    var id: String { rawValue }

    var title: String {
        switch self {
        case .a: return "A / Cruz"
        case .b: return "B / Círculo"
        case .x: return "X / Cuadrado"
        case .y: return "Y / Triángulo"
        case .leftShoulder: return "LB / L1"
        case .rightShoulder: return "RB / R1"
        case .dpadUp: return "Cruceta arriba"
        case .dpadDown: return "Cruceta abajo"
        case .dpadLeft: return "Cruceta izquierda"
        case .dpadRight: return "Cruceta derecha"
        case .menu: return "Menú / Options"
        case .leftStick: return "Clic stick izquierdo (L3)"
        case .rightStick: return "Clic stick derecho (R3)"
        }
    }
}

enum PadAction: String, CaseIterable, Identifiable {
    case legs3, legs2, waddle, legsStop, stopAll, setForward, randomSound, randomEmote, toggleMovie, toggleMonkeys, turbo, boost, aimToggle, domeCenter
    var id: String { rawValue }

    var title: String {
        switch self {
        case .legs3: return "3 patas"
        case .legs2: return "2 patas"
        case .waddle: return "Contoneo"
        case .legsStop: return "Parar patas"
        case .stopAll: return "Parar todo"
        case .setForward: return "Fijar «adelante»"
        case .randomSound: return "Sonido al azar"
        case .randomEmote: return "Emote al azar"
        case .toggleMovie: return "Vamos a ver una película (activar/apagar)"
        case .toggleMonkeys: return "Haz monerías (activar/apagar)"
        case .turbo: return "Turbo (mientras se mantiene)"
        case .boost: return "Impulso (BB-8)"
        case .aimToggle: return "Apuntar / fijar «adelante» (alternar)"
        case .domeCenter: return "Centrar la cúpula"
        }
    }
}

/// Controles con mando:
/// - Stick izquierdo: mueve el droide (en modo apuntado, lo orienta sin avanzar).
/// - Stick derecho (eje X): la cúpula gira poco a poco mientras se mantiene; al soltarlo se queda donde está.
/// - RT sube la velocidad máxima y LT la baja.
/// - Botones: asignables; por defecto A = 3 patas, B = 2 patas, Y = contoneo, X = parar patas,
///   clic del stick izquierdo = apuntar / fijar «adelante» y clic del derecho = centrar la cúpula.
final class GamepadController: ObservableObject {
    @Published var connectedName: String?
    @Published var mapping: [PadButton: PadAction]
    @Published var maxSpeed = 0.6
    @Published var dome = 0.0
    @Published var turbo = false
    /// Modo apuntado activo: el stick izquierdo orienta el droide sin moverlo.
    @Published var aiming = false

    private static let defaultsKey = "padMapping"
    private static let versionKey = "padMappingVersion"
    private static let defaultMapping: [PadButton: PadAction] = [
        .a: .legs3, .b: .legs2, .y: .waddle, .x: .legsStop, .leftStick: .aimToggle, .rightStick: .domeCenter,
    ]

    private weak var droid: Droid?
    private var loop: EmoteLoopController?
    private var stopAll: (() -> Void)?
    private var timer: Timer?
    private var held = Set<PadButton>()
    private var lastTick = Date()
    private var lastDrive = Date.distantPast
    private var lastDomeSent = Date.distantPast
    private var heading = 0
    private var driving = false
    private var domeDirty = false

    init() {
        mapping = Self.load()
        // Sin esto iOS no entrega las pulsaciones del mando a una app en segundo plano (pantalla bloqueada).
        GCController.shouldMonitorBackgroundEvents = true
        let nc = NotificationCenter.default
        nc.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] _ in self?.refreshName() }
        nc.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] _ in self?.refreshName() }
        refreshName()
    }

    // MARK: Ciclo de vida

    func start(droid: Droid, loop: EmoteLoopController, stopAll: @escaping () -> Void) {
        self.droid = droid; self.loop = loop; self.stopAll = stopAll
        timer?.invalidate()
        lastTick = Date()
        timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in self?.tick() }
    }

    func stop() {
        timer?.invalidate(); timer = nil
        if driving { droid?.stop(heading: heading) }
        driving = false
        if aiming { droid?.setAimLight(false); aiming = false }
    }

    private func refreshName() {
        let c = GCController.controllers().first
        connectedName = c.map { $0.vendorName ?? $0.productCategory }
    }

    // MARK: Asignación

    func setAction(_ a: PadAction?, for b: PadButton) {
        if let a { mapping[b] = a } else { mapping.removeValue(forKey: b) }
        Self.save(mapping)
    }

    func resetMapping() {
        mapping = Self.defaultMapping
        Self.save(mapping)
    }

    private static func load() -> [PadButton: PadAction] {
        let defaults = UserDefaults.standard
        guard let raw = defaults.dictionary(forKey: defaultsKey) as? [String: String] else {
            defaults.set(2, forKey: versionKey)
            return defaultMapping
        }
        var map: [PadButton: PadAction] = [:]
        for (k, v) in raw { if let b = PadButton(rawValue: k), let a = PadAction(rawValue: v) { map[b] = a } }
        // Versión 2: los clics de los sticks pasan a apuntar y centrar la cúpula. Solo si el usuario no los había asignado.
        if defaults.integer(forKey: versionKey) < 2 {
            for b in [PadButton.leftStick, .rightStick] where map[b] == nil { map[b] = defaultMapping[b] }
            defaults.set(2, forKey: versionKey)
            save(map)
        }
        return map
    }

    private static func save(_ m: [PadButton: PadAction]) {
        UserDefaults.standard.set(Dictionary(uniqueKeysWithValues: m.map { ($0.key.rawValue, $0.value.rawValue) }), forKey: defaultsKey)
    }

    // MARK: Lectura

    /// Zona muerta radial con reescalado, para que el stick no tiemble en reposo.
    static func deadzone(_ x: Float, _ y: Float) -> (Float, Float) {
        let dead: Float = 0.15
        let m = hypot(x, y)
        guard m >= dead else { return (0, 0) }
        let scaled = min((m - dead) / (1 - dead), 1)
        return (x / m * scaled, y / m * scaled)
    }

    /// Rumbo (0…359; 0 = adelante, sentido horario) con y positiva hacia arriba.
    static func headingOf(_ x: Float, _ y: Float) -> Int {
        (Int((atan2(x, y) * 180 / .pi).rounded()) % 360 + 360) % 360
    }

    /// Nueva posición de la cúpula tras mantener el stick `x` durante `dt` segundos (-160°…180°).
    /// La curva cuadrática da precisión cerca del centro; a tope barre unos 120°/s.
    static func domeStep(_ current: Double, _ x: Float, _ dt: Double) -> Double {
        min(180, max(-160, current + Double(x * abs(x)) * 120 * dt))
    }

    private func pressedButtons(_ gp: GCExtendedGamepad) -> Set<PadButton> {
        var s = Set<PadButton>()
        if gp.buttonA.isPressed { s.insert(.a) }
        if gp.buttonB.isPressed { s.insert(.b) }
        if gp.buttonX.isPressed { s.insert(.x) }
        if gp.buttonY.isPressed { s.insert(.y) }
        if gp.leftShoulder.isPressed { s.insert(.leftShoulder) }
        if gp.rightShoulder.isPressed { s.insert(.rightShoulder) }
        if gp.dpad.up.isPressed { s.insert(.dpadUp) }
        if gp.dpad.down.isPressed { s.insert(.dpadDown) }
        if gp.dpad.left.isPressed { s.insert(.dpadLeft) }
        if gp.dpad.right.isPressed { s.insert(.dpadRight) }
        if gp.buttonMenu.isPressed { s.insert(.menu) }
        if gp.leftThumbstickButton?.isPressed == true { s.insert(.leftStick) }
        if gp.rightThumbstickButton?.isPressed == true { s.insert(.rightStick) }
        return s
    }

    private func tick() {
        guard let d = droid, d.isReady, let gp = GCController.controllers().first?.extendedGamepad else { return }
        let now = Date()
        let dt = now.timeIntervalSince(lastTick)
        lastTick = now

        // Gatillos: RT sube y LT baja la velocidad máxima.
        // Solo se publica si cambia: reasignar el mismo valor redibuja la pantalla y cierra los menús abiertos.
        let trigger = Double(gp.rightTrigger.value - gp.leftTrigger.value)
        if trigger != 0 {
            let v = min(1, max(0.15, maxSpeed + trigger * 0.5 * dt))
            if v != maxSpeed { maxSpeed = v }
        }

        let current = pressedButtons(gp)
        let newly = current.subtracting(held)
        held = current
        let turboNow = current.contains { mapping[$0] == .turbo }
        if turboNow != turbo { turbo = turboNow }

        // Stick izquierdo: mueve el droide; en modo apuntado lo orienta sin avanzar.
        let (lx, ly) = Self.deadzone(gp.leftThumbstick.xAxis.value, gp.leftThumbstick.yAxis.value)
        let mag = hypot(lx, ly)
        if aiming {
            if mag > 0 {
                heading = Self.headingOf(lx, ly)
                if now.timeIntervalSince(lastDrive) >= 0.1 { lastDrive = now; d.aim(heading: heading) }
            }
        } else if mag > 0 {
            heading = Self.headingOf(lx, ly)
            driving = true
            if now.timeIntervalSince(lastDrive) >= 0.1 {
                lastDrive = now
                d.drive(speed: Int(Double(mag) * 255 * (turbo ? 1 : maxSpeed)), heading: heading)
            }
        } else if driving {
            driving = false
            d.stop(heading: heading)
        }

        // Stick derecho (eje X): la cúpula gira poco a poco y se queda donde se suelta.
        if d.kind.hasDome {
            let (rx, _) = Self.deadzone(gp.rightThumbstick.xAxis.value, 0)
            if rx != 0 {
                let target = Self.domeStep(dome, rx, dt)
                if target != dome { dome = target; domeDirty = true }
            }
            if domeDirty, now.timeIntervalSince(lastDomeSent) >= 0.1 {
                lastDomeSent = now
                domeDirty = false
                d.setDome(Float(dome))
            }
        }

        for b in newly { if let a = mapping[b] { run(a, on: d) } }
    }

    /// Primera pulsación: modo apuntado (luz de apuntado y el stick orienta el droide).
    /// Segunda: la orientación actual se fija como «adelante».
    private func toggleAim(on d: Droid) {
        if aiming {
            d.setForward()
            d.setAimLight(false)
            aiming = false
            heading = 0
        } else {
            if driving { d.stop(heading: heading); driving = false }
            aiming = true
            d.setAimLight(true)
        }
    }

    private func run(_ a: PadAction, on d: Droid) {
        switch a {
        case .legs3: d.legAction(.threeLegs)
        case .legs2: d.legAction(.twoLegs)
        case .waddle: d.legAction(.waddle)
        case .legsStop: d.legAction(.stop)
        case .stopAll: stopAll?()
        case .setForward: d.setForward()
        case .randomSound:
            let pool = d.kind.sounds.filter { ["Chatty", "Excited", "Positive", "Hey"].contains($0.group) }
            if let s = pool.randomElement() { d.play(s) }
        case .randomEmote:
            if let loop, !d.busy, let a = loop.pool(d, .emotions).randomElement() { d.play(a) }
        case .toggleMovie: toggle(.wwm, on: d)
        case .toggleMonkeys: toggle(.emotions, on: d)
        case .boost: d.boostV1(heading: heading)
        case .turbo: break   // se aplica mientras se mantiene pulsado
        case .aimToggle: toggleAim(on: d)
        case .domeCenter:
            dome = 0
            domeDirty = false
            d.setDome(0)
        }
    }

    private func toggle(_ mode: EmoteLoopController.Mode, on d: Droid) {
        guard let loop else { return }
        if loop.running == mode { loop.stop() } else { loop.start(mode, on: d) }
    }
}

struct GamepadView: View {
    @ObservedObject var pad: GamepadController

    var body: some View {
        Form {
          Group {
            Section {
                if let n = pad.connectedName {
                    Label("Mando conectado: \(n)", systemImage: "gamecontroller.fill").foregroundStyle(.green)
                } else {
                    Label("Sin mando", systemImage: "gamecontroller").foregroundStyle(.secondary)
                    Text("Empareja un mando Xbox o PlayStation en Ajustes › Bluetooth.").font(.footnote).foregroundStyle(.secondary)
                }
            }
            Section("Controles fijos") {
                Text("Stick izquierdo: mueve el droide.")
                Text("Stick derecho (eje X): la cúpula gira poco a poco mientras lo mantienes y se queda donde la sueltas. Clic del stick derecho: la centra.")
                Text("RT: sube la velocidad máxima · LT: la baja. Ahora: \(Int(pad.maxSpeed * 100)) %.")
                Text("Clic del stick izquierdo: modo apuntado (el stick orienta el droide sin moverlo); púlsalo otra vez para fijar esa orientación como «adelante».")
                if pad.aiming {
                    Label("Modo apuntado activo", systemImage: "scope").foregroundStyle(.orange)
                }
                Text("En PlayStation: Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT.")
                    .font(.footnote).foregroundStyle(.secondary)
            }
            Section("Botones") {
                ForEach(PadButton.allCases) { b in
                    Picker(b.title, selection: Binding<PadAction?>(get: { pad.mapping[b] }, set: { pad.setAction($0, for: b) })) {
                        Text("Sin asignar").tag(PadAction?.none)
                        ForEach(PadAction.allCases) { Text($0.title).tag(PadAction?.some($0)) }
                    }
                    .pickerStyle(.menu)
                }
                Button("Restablecer (A = 3 patas, B = 2 patas, Y = contoneo, X = parar, clics de stick = apuntar y centrar)", action: pad.resetMapping)
            }
          }
          .listRowBackground(Theme.card)
        }
        .themedList()
    }
}
