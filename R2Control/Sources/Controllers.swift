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
                    log("Alarma: \(a.name)")
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
        lastPlayed = anim.name
        nextAt = Date().addingTimeInterval(mode == .wwm ? wwmInterval : emotionsInterval)
    }
}
