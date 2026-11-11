import Combine
import CoreGraphics
import Foundation

// MARK: - Patrulla

/// Modo patrulla: el droide recorre el espacio solo, reacciona a los golpes y suelta alarmas,
/// y se guarda un registro de eventos como en la app original.
final class PatrolController: ObservableObject {
    struct Entry: Identifiable { let id = UUID(); let time: Date; let text: String }
    private enum Phase { case forward, backing(Date), settling(Date), alarm(Date) }

    @Published var running = false
    @Published var entries: [Entry] = []
    @Published var speed: Double = 70

    private weak var droid: Droid?
    private var timer: Timer?
    private var hitSub: AnyCancellable?
    private var phase: Phase = .forward
    private var heading = 0
    private var nextAlarm = Date()

    func start(_ d: Droid) {
        stop()
        droid = d
        heading = 0
        running = true
        phase = .forward
        nextAlarm = Date().addingTimeInterval(.random(in: 20...35))
        d.configureCollisions()
        log("Patrulla iniciada con \(d.name)")
        hitSub = d.collision.sink { [weak self] in self?.onHit() }
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.tick() }
    }

    func stop() {
        timer?.invalidate(); timer = nil
        hitSub = nil
        if running { droid?.stop(heading: heading); log("Patrulla detenida") }
        running = false
    }

    private func log(_ s: String) { entries.insert(Entry(time: Date(), text: s), at: 0) }

    private func onHit() {
        guard running, case .forward = phase else { return }
        log("Colisión: retrocediendo")
        phase = .backing(Date().addingTimeInterval(0.8))
    }

    private func tick() {
        guard let d = droid, d.isReady else { stop(); return }
        let now = Date()
        switch phase {
        case .forward:
            if now >= nextAlarm {
                d.stop(heading: heading)
                let pool = d.kind.animations.filter { $0.category == "Patrol" }
                let anim = pool.randomElement() ?? d.kind.animations.filter { $0.category == "Emote" }.randomElement()
                if let a = anim {
                    d.play(a)
                    log("Alarma: \(Spanish.title(a.name))")
                    phase = .alarm(now.addingTimeInterval(a.seconds + 0.5))
                } else {
                    phase = .settling(now.addingTimeInterval(0.5))
                }
                nextAlarm = now.addingTimeInterval(.random(in: 20...40))
            } else {
                d.drive(speed: Int(speed), heading: heading)
            }
        case let .backing(until):
            d.drive(speed: Int(speed * 0.6), heading: heading, reverse: true)
            if now >= until {
                let turn = Int.random(in: 100...200) * (Bool.random() ? 1 : -1)
                heading = ((heading + turn) % 360 + 360) % 360
                log("Giro de \(turn)°")
                phase = .settling(now.addingTimeInterval(0.8))
            }
        case let .settling(until):
            d.drive(speed: 0, heading: heading)
            if now >= until { phase = .forward }
        case let .alarm(until):
            if now >= until { phase = .forward }
        }
    }
}

// MARK: - Draw and Drive

/// Dibujas un recorrido en pantalla y el droide lo sigue. Sin sensores de posición, se calcula por tiempo:
/// la distancia recorrida depende de la calibración de velocidad.
final class DrawDriveController: ObservableObject {
    struct Segment { let heading: Int; let seconds: Double; let speed: Int }

    @Published var points: [CGPoint] = []
    @Published var running = false
    @Published var progress = 0.0
    @Published var speed: Double = 80
    @Published var cmPerSecond: Double = 30     // calibración: cm/s a esa velocidad
    @Published var cmPerPoint: Double = 0.4     // escala del dibujo

    private weak var droid: Droid?
    private var timer: Timer?
    private var hitSub: AnyCancellable?
    private var segments: [Segment] = []
    private var started = Date()
    private var total = 0.0

    func clear() { abort(); points = [] }

    func run(_ d: Droid) {
        abort()
        segments = buildSegments()
        total = segments.reduce(0) { $0 + $1.seconds }
        guard total > 0 else { return }
        droid = d
        d.configureCollisions()
        hitSub = d.collision.sink { [weak self] in self?.abort() }
        started = Date()
        running = true
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.tick() }
    }

    func abort() {
        timer?.invalidate(); timer = nil
        hitSub = nil
        if running { droid?.stop() }
        running = false
        progress = 0
    }

    private func tick() {
        guard let d = droid, d.isReady else { abort(); return }
        let t = Date().timeIntervalSince(started)
        progress = min(1, t / total)
        var acc = 0.0
        for s in segments {
            if t < acc + s.seconds { d.drive(speed: s.speed, heading: s.heading); return }
            acc += s.seconds
        }
        abort()
    }

    /// Simplifica el trazo y lo convierte en tramos (rumbo, duración).
    private func buildSegments() -> [Segment] {
        var pts: [CGPoint] = []
        for p in points where pts.last.map({ hypot($0.x - p.x, $0.y - p.y) >= 14 }) ?? true { pts.append(p) }
        guard pts.count > 1 else { return [] }

        var raw: [(Double, Double)] = []   // (rumbo, distancia en puntos)
        for (a, b) in zip(pts, pts.dropFirst()) {
            var deg = atan2(Double(b.x - a.x), Double(-(b.y - a.y))) * 180 / .pi
            if deg < 0 { deg += 360 }
            raw.append((deg, hypot(Double(b.x - a.x), Double(b.y - a.y))))
        }
        // Une tramos casi alineados.
        var merged: [(Double, Double)] = []
        for r in raw {
            if let last = merged.last, abs(angleDiff(last.0, r.0)) < 15 { merged[merged.count - 1].1 += r.1 }
            else { merged.append(r) }
        }
        var out: [Segment] = []
        var prev = merged[0].0
        for m in merged {
            if abs(angleDiff(prev, m.0)) > 30 { out.append(Segment(heading: Int(m.0), seconds: 0.4, speed: 0)) }
            out.append(Segment(heading: Int(m.0), seconds: m.1 * cmPerPoint / cmPerSecond, speed: Int(speed)))
            prev = m.0
        }
        return out
    }

    private func angleDiff(_ a: Double, _ b: Double) -> Double {
        var d = (b - a).truncatingRemainder(dividingBy: 360)
        if d > 180 { d -= 360 } else if d < -180 { d += 360 }
        return d
    }
}

// MARK: - Emotes automáticos

/// Lanza emotes al azar cada cierto número de segundos. Solo hay un modo activo a la vez.
final class EmoteLoopController: ObservableObject {
    enum Mode { case wwm, emotions }

    @Published var running: Mode?
    @Published var wwmInterval = 15.0
    @Published var emotionsInterval = 15.0
    @Published var includeIonBlast = false
    @Published var lastPlayed = ""

    private weak var droid: Droid?
    private var timer: Timer?
    private var nextAt = Date()
    private var last: UInt16?

    deinit { timer?.invalidate() }

    static func isIonBlast(_ a: DroidAnimation) -> Bool { a.name.localizedCaseInsensitiveContains("IonBlast") }

    func pool(_ droid: Droid, _ mode: Mode) -> [DroidAnimation] {
        switch mode {
        case .wwm: return droid.kind.animations.filter { $0.category == "WWM" }
        case .emotions: return droid.kind.animations.filter { $0.category == "Emote" && (includeIonBlast || !Self.isIonBlast($0)) }
        }
    }

    func start(_ mode: Mode, on d: Droid) {
        stop()
        droid = d
        running = mode
        nextAt = Date()
        timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in self?.tick() }
    }

    /// Para el bucle. La animación en curso termina sola.
    func stop() {
        timer?.invalidate(); timer = nil
        running = nil
    }

    private func tick() {
        guard let d = droid, d.isReady, let mode = running else { stop(); return }
        // Si la animación anterior dura más que el intervalo, se espera a que acabe.
        guard Date() >= nextAt, !d.busy else { return }
        let all = pool(d, mode)
        guard let anim = (all.count > 1 ? all.filter { $0.index != last } : all).randomElement() else { return }
        d.play(anim)
        last = anim.index
        lastPlayed = Spanish.title(anim.name)
        nextAt = Date().addingTimeInterval(mode == .wwm ? wwmInterval : emotionsInterval)
    }
}

// MARK: - Seguir al móvil

/// Sigue al iPhone usando solo la intensidad de la señal Bluetooth (RSSI).
///
/// El RSSI no da dirección y fluctúa mucho. En vez de reaccionar a medidas sueltas, el droide avanza
/// zigzagueando y apunta cada medida junto a su posición estimada (se calcula con las órdenes enviadas,
/// sin sensores). Con las medidas de los últimos segundos se ajusta un plano: su pendiente indica hacia
/// dónde crece la señal, y hacia ahí se corrige el rumbo. El zigzag hace falta para ver también la
/// pendiente lateral. Frena al acercarse, se para si deja de recibir medidas y sigue siendo aproximado.
/// Es el mismo algoritmo que `Follow` en la app de escritorio (desktop/src/controllers.rs), que lo prueba
/// con una simulación.
final class FollowController: ObservableObject {
    private enum Phase: Equatable { case forward, settling(Date), arrived }
    struct Sample { let x: Double, y: Double, rssi: Double }

    @Published var running = false
    @Published var smoothed: Double?
    @Published var status = "Detenido"
    @Published var nearThreshold = -55.0   // dBm; por encima de esto se considera «ya he llegado»
    @Published var speed = 45.0
    /// Últimas señales filtradas, para la gráfica.
    @Published var history: [Double] = []
    static let historySize = 160

    private let leg = 1.8             // segundos de cada tramo del zigzag
    private let weave = 30.0          // grados que se desvía cada tramo a un lado y a otro del rumbo
    private let fitWindow = 36        // medidas que se usan para estimar la pendiente (unos 12 s)
    private let fitMinStrength = 0.8  // dB que tiene que explicar la pendiente para fiarse de ella
    private let lostLevel = -85.0     // dBm: por debajo vuelve sobre sus pasos y, si sigue igual, se para
    private let stale = 2.5           // segundos sin medidas tras los que el droide se para
    private let hysteresis = 5.0      // dB que tiene que alejarse la señal para volver a moverse tras llegar

    private weak var droid: Droid?
    private var timer: Timer?
    private var rssiSub: AnyCancellable?
    private var hitSub: AnyCancellable?
    private var phase = Phase.forward
    private var target = 0.0          // rumbo hacia el que se cree que está el móvil
    private var weaveSign = 1.0
    private var heading = 0
    private var command = (speed: 0, heading: 0)
    private var pos = (x: 0.0, y: 0.0)
    private var lastTick = Date()
    private var samples: [Sample] = []
    private var recent: [Double] = []
    private var lastSampleAt: Date?
    private var legEnd = Date()
    private var retreated = false
    private var stalled = false
    private var ticks = 0

    deinit { timer?.invalidate() }

    /// Empieza a escuchar la señal de este droide. La señal se sigue aunque el modo esté parado,
    /// para poder verla y calibrar «cerca» sin que el droide se mueva.
    func attach(_ d: Droid) {
        guard droid !== d || rssiSub == nil else { return }
        droid = d
        smoothed = nil; recent = []; history = []; lastSampleAt = nil
        rssiSub = d.$rssi.compactMap { $0 }.sink { [weak self] v in self?.ingest(Double(v)) }
    }

    /// Hay medidas recientes de señal.
    func isFresh(at now: Date = Date()) -> Bool {
        lastSampleAt.map { now.timeIntervalSince($0) < stale } ?? false
    }

    func start(_ d: Droid) {
        stop()
        attach(d)
        running = true
        phase = .forward
        target = 0; weaveSign = 1
        pos = (0, 0)
        samples = []
        retreated = false; stalled = false
        lastTick = Date()
        newLeg(lastTick)
        status = "Esperando señal…"
        d.configureCollisions()
        hitSub = d.collision.sink { [weak self] in self?.onHit() }
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.tick() }
    }

    func stop() {
        timer?.invalidate(); timer = nil
        hitSub = nil
        if running { droid?.stop(heading: heading) }
        command = (0, heading)
        running = false
        status = "Detenido"
    }

    /// Deja el droide a la distancia a la que quieres que se pare y úsalo para fijar qué señal significa «cerca».
    func calibrateNear() {
        if let s = smoothed { nearThreshold = min(max((s - 3).rounded(), -80), -35) }
    }

    /// Ajusta un plano a las medidas y devuelve (rumbo hacia el que crece la señal, dB que explica).
    static func gradient(_ samples: [Sample]) -> (bearing: Double, strength: Double)? {
        let n = Double(samples.count)
        guard samples.count >= 10 else { return nil }
        let mx = samples.reduce(0) { $0 + $1.x } / n
        let my = samples.reduce(0) { $0 + $1.y } / n
        let mr = samples.reduce(0) { $0 + $1.rssi } / n
        var sxx = 0.0, sxy = 0.0, syy = 0.0, sxr = 0.0, syr = 0.0
        for s in samples {
            let dx = s.x - mx, dy = s.y - my, dr = s.rssi - mr
            sxx += dx * dx; sxy += dx * dy; syy += dy * dy; sxr += dx * dr; syr += dy * dr
        }
        let spread = sxx + syy
        guard spread > 1e-3 else { return nil }
        // Regularización: si el recorrido es casi una línea, la pendiente lateral se queda en cero en vez de dispararse.
        let ridge = 0.08 * spread
        let a = sxx + ridge, d = syy + ridge
        let det = a * d - sxy * sxy
        let gx = (d * sxr - sxy * syr) / det
        let gy = (a * syr - sxy * sxr) / det
        var bearing = atan2(gx, gy) * 180 / .pi
        if bearing < 0 { bearing += 360 }
        return (bearing, hypot(gx, gy) * (spread / n).squareRoot())
    }

    private static func norm(_ a: Double) -> Double {
        let r = a.truncatingRemainder(dividingBy: 360)
        return r < 0 ? r + 360 : r
    }

    /// Diferencia b − a en (−180, 180].
    private static func angleDiff(_ a: Double, _ b: Double) -> Double {
        var d = (b - a).truncatingRemainder(dividingBy: 360)
        if d > 180 { d -= 360 } else if d < -180 { d += 360 }
        return d
    }

    private func newLeg(_ now: Date) {
        legEnd = now.addingTimeInterval(leg)
        weaveSign = -weaveSign
        heading = Int(Self.norm(target + weave * weaveSign))
    }

    private func go(_ d: Droid, _ speed: Int) {
        command = (speed, heading)
        d.drive(speed: speed, heading: heading)
    }

    private func halt(_ d: Droid) {
        command = (0, heading)
        d.stop(heading: heading)
    }

    /// Cambio brusco de rumbo: gira sobre sí mismo un momento antes de seguir.
    private func turn(to newTarget: Double, _ now: Date, seconds: Double) {
        target = Self.norm(newTarget)
        heading = Int(target)
        phase = .settling(now.addingTimeInterval(seconds))
    }

    /// Medida nueva: mediana de 3 (quita picos) y media exponencial.
    private func ingest(_ v: Double) {
        lastSampleAt = Date()
        recent.append(v)
        if recent.count > 3 { recent.removeFirst() }
        let median = recent.sorted()[recent.count / 2]
        let s = smoothed.map { 0.65 * $0 + 0.35 * median } ?? median
        smoothed = s
        history.append(s)
        if history.count > Self.historySize { history.removeFirst() }
        if running, phase == .forward {
            // Para el ajuste se usa la medida sin suavizar: suavizarla la retrasaría respecto a la posición.
            samples.append(Sample(x: pos.x, y: pos.y, rssi: v))
            if samples.count > fitWindow { samples.removeFirst() }
        }
    }

    private func onHit() {
        guard running else { return }
        samples = []
        turn(to: Double(heading + Int.random(in: 120...240)), Date(), seconds: 1.0)
        status = "Obstáculo: cambio de rumbo"
    }

    private func tick() {
        guard let d = droid, d.isReady else { stop(); return }
        let now = Date()
        let dt = min(now.timeIntervalSince(lastTick), 0.5)
        lastTick = now
        // Posición estimada: lo que se le ha mandado hacer desde el tick anterior.
        if command.speed > 0 {
            let a = Double(command.heading) * .pi / 180
            let step = Double(command.speed) * dt
            pos = (pos.x + sin(a) * step, pos.y + cos(a) * step)
        }
        ticks += 1
        if ticks % 3 == 0 { d.readRSSI() }

        // Sin medidas no se puede decidir nada: mejor quieto que avanzando a ciegas.
        guard let at = lastSampleAt, now.timeIntervalSince(at) < stale else {
            if lastSampleAt != nil {
                if !stalled { stalled = true; halt(d) }
                status = "Sin lectura de señal: esperando…"
            }
            return
        }
        if stalled { stalled = false; newLeg(now) }
        guard let s = smoothed else { return }

        switch phase {
        case .arrived:
            status = "He llegado (\(Int(s)) dBm)"
            if s < nearThreshold - hysteresis { phase = .forward; samples = []; newLeg(now) }
        case let .settling(until):
            go(d, 0)   // gira sobre sí mismo hacia el rumbo nuevo
            if now >= until { phase = .forward; newLeg(now) }
        case .forward:
            if s >= nearThreshold {
                phase = .arrived
                halt(d)
                status = "He llegado (\(Int(s)) dBm)"
                return
            }
            if s <= lostLevel {
                if retreated {
                    stop()
                    status = "Señal muy débil (\(Int(s)) dBm): parado para no perderlo"
                } else {
                    // Una oportunidad: deshacer el camino antes de rendirse.
                    retreated = true
                    samples = []
                    turn(to: target + 180, now, seconds: 0.7)
                    status = "Señal muy débil: vuelvo sobre mis pasos"
                }
                return
            }
            if s > lostLevel + 6 { retreated = false }
            // Frena al acercarse para no pasarse de largo.
            let slow = min(max((nearThreshold - s) / 10, 0.5), 1.0)
            go(d, Int(speed * slow))
            guard now >= legEnd else { return }
            status = "Acercándome… (\(Int(s)) dBm)"
            if let g = Self.gradient(samples), g.strength >= fitMinStrength {
                let diff = Self.angleDiff(target, g.bearing)
                if abs(diff) > 100 {
                    turn(to: g.bearing, now, seconds: 0.7)
                    status = "La señal cae: media vuelta"
                    return
                }
                target = Self.norm(target + 0.7 * diff)
                if abs(diff) > 20 { status = "Corrigiendo rumbo (\(Int(s)) dBm)" }
            }
            newLeg(now)
        }
    }
}
