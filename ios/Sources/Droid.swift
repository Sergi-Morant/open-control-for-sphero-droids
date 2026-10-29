import Combine
import CoreBluetooth
import Foundation

enum BatteryLevel: String {
    case unknown = "—", charged = "Cargado", charging = "Cargando", ok = "Normal", low = "Baja", critical = "Crítica"
    var symbol: String {
        switch self {
        case .unknown: return "battery.0"
        case .charged, .charging: return "battery.100.bolt"
        case .ok: return "battery.75"
        case .low: return "battery.25"
        case .critical: return "battery.0"
        }
    }
}

enum LegAction: UInt8, CaseIterable, Identifiable {
    case stop = 0, threeLegs, twoLegs, waddle
    var id: UInt8 { rawValue }
    var title: String { ["Parar", "3 patas", "2 patas", "Contoneo"][Int(rawValue)] }
}

enum InteractionMode: Int, CaseIterable, Identifiable {
    case off, idle, social
    var id: Int { rawValue }
    var title: String { ["Ninguna", "Animaciones de reposo", "Reposo + otros droides"][rawValue] }
}

/// Un droide concreto: conexión BLE, protocolo (v1 o v2) y comandos de alto nivel.
/// Todos los IDs de dispositivo/comando salen del SDK de la app original (ver tools/sdk_commands.txt).
final class Droid: NSObject, ObservableObject, Identifiable {
    enum Link { case connecting, ready, disconnected }

    // API v2 (R2-D2, R2-Q5, BB-9E)
    static let v2Api = CBUUID(string: "00010002-574F-4F20-5370-6865726F2121")
    static let v2AntiDos = CBUUID(string: "00020005-574F-4F20-5370-6865726F2121")
    static let v2Secret = "usetheforce...band"
    // API v1 (BB-8)
    static let v1AntiDos = CBUUID(string: "22BB746F-2BBD-7554-2D6F-726568705327")
    static let v1TxPower = CBUUID(string: "22BB746F-2BB2-7554-2D6F-726568705327")
    static let v1Wake = CBUUID(string: "22BB746F-2BBF-7554-2D6F-726568705327")
    static let v1Command = CBUUID(string: "22BB746F-2BA1-7554-2D6F-726568705327")
    static let v1Response = CBUUID(string: "22BB746F-2BA6-7554-2D6F-726568705327")
    static let v1Secret = "011i3"

    let kind: DroidKind
    let peripheral: CBPeripheral
    let name: String
    var id: UUID { peripheral.identifier }

    @Published var link: Link = .connecting
    @Published var status = "Conectando…"
    @Published var log: [String] = []
    @Published var batteryLevel: BatteryLevel = .unknown
    @Published var batteryText = "—"
    @Published var versions: [String: String] = [:]
    @Published var routingText = "sin detectar"
    @Published var busy = false                 // reproduciendo una animación
    @Published var rssi: Int?                   // señal Bluetooth medida desde el iPhone (dBm)
    @Published var chatter = false
    @Published var reactToHits = true
    @Published var idleAnimations = false
    @Published var trophyMode = false
    @Published var headTracking = false

    let collision = PassthroughSubject<Void, Never>()
    var isReady: Bool { link == .ready }

    private var chars: [CBUUID: CBCharacteristic] = [:]
    private var pendingServices = 0
    private var setupStep = 0
    private var routing = Routing.candidates[0]
    private var routingConfirmed = false
    private var probes: [UInt8: Routing] = [:]
    private var seq: UInt8 = 0
    private var v2Decoder = PacketDecoder()
    private var v1Decoder = V1Decoder()
    private var v1BatterySeq: UInt8 = 0
    private var pollTimer: Timer?
    private var chatterTimer: Timer?
    private var busyWork: DispatchWorkItem?
    private var cancellables = Set<AnyCancellable>()

    init(kind: DroidKind, peripheral: CBPeripheral, name: String) {
        self.kind = kind
        self.peripheral = peripheral
        self.name = name
        super.init()
        peripheral.delegate = self
        collision.sink { [weak self] in self?.handleHit() }.store(in: &cancellables)
    }

    private static let clock: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()

    func note(_ s: String) {
        log.append("\(Self.clock.string(from: Date())) \(s)")
        if log.count > 600 { log.removeFirst(200) }
    }

    /// Códigos de error de la API v2 (el 8 se observó en un R2-D2 real al conducir durante una animación).
    private static let errorNames: [UInt8: String] = [
        1: "DID no válido", 2: "CID no válido", 3: "no implementado", 4: "restringido", 5: "longitud de datos incorrecta",
        6: "falló", 7: "parámetro no válido", 8: "ocupado", 9: "destino no válido", 10: "destino no disponible",
    ]
    private var lastError = ""
    private var lastErrorAt = Date.distantPast

    private func noteError(_ e: UInt8, did: UInt8, cid: UInt8) {
        let key = String(format: "%02X/%02X e%d", did, cid, e)
        let now = Date()
        guard key != lastError || now.timeIntervalSince(lastErrorAt) > 3 else { return }  // no repetir al conducir a 10 Hz
        lastError = key; lastErrorAt = now
        note("✗ error \(e) (\(Self.errorNames[e] ?? "?")) en \(String(format: "%02X/%02X", did, cid))")
    }

    // MARK: - Ciclo de vida (lo llama DroidManager)

    func didConnect() {
        status = "Descubriendo servicios…"
        peripheral.discoverServices(nil)
    }

    func didDisconnect() {
        link = .disconnected
        status = "Desconectado"
        pollTimer?.invalidate(); chatterTimer?.invalidate()
        chatter = false
        chars = [:]
        setupStep = 0
        routingConfirmed = false
    }

    private func becameReady() {
        link = .ready
        status = "Conectado"
        pollTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in self?.requestBattery() }
        if kind.usesV2 {
            wake()
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in self?.startProbe() }
        } else {
            afterRoutingConfirmed()
        }
    }

    // MARK: - Detección de direcciones v2

    private func startProbe() {
        guard link == .ready, !routingConfirmed else { return }
        var list = Routing.candidates
        if let saved = Self.savedRouting(for: kind), let i = list.firstIndex(of: saved) {
            list.remove(at: i); list.insert(saved, at: 0)
        }
        for (i, cand) in list.enumerated() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5 * Double(i)) { [weak self] in
                guard let self, self.link == .ready, !self.routingConfirmed else { return }
                self.seq &+= 1
                self.probes[self.seq] = cand
                let pkt = SpheroPacket(routing: cand, toST: true, did: Device.apiProcessor, cid: 0x00, seq: self.seq)
                self.writeAPI(pkt.encoded())
                self.note("probe \(cand.title)")
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5 * Double(list.count) + 1.5) { [weak self] in
            guard let self, self.link == .ready, !self.routingConfirmed else { return }
            self.routingText = "no se pudo detectar"
            self.note("⚠︎ Ningún esquema de direcciones obtuvo respuesta del procesador principal")
        }
    }

    private static func savedRouting(for kind: DroidKind) -> Routing? {
        guard let d = UserDefaults.standard.data(forKey: "routing.\(kind.rawValue)") else { return nil }
        return try? JSONDecoder().decode(Routing.self, from: d)
    }

    private func afterRoutingConfirmed() {
        wake()
        configureCollisions()
        if kind.usesV2 {
            send(st: false, Device.power, 0x05, [1])    // notificaciones de batería
            requestVersions()
        } else {
            send1(0x00, 0x21, [1])                      // notificaciones de energía (v1)
        }
        requestBattery()
    }

    // MARK: - Envío

    private func writeAPI(_ data: Data) {
        let c = kind.usesV2 ? chars[Self.v2Api] : chars[Self.v1Command]
        guard let c else { return }
        let type: CBCharacteristicWriteType = c.properties.contains(.writeWithoutResponse) ? .withoutResponse : .withResponse
        peripheral.writeValue(data, for: c, type: type)
    }

    private func send(st: Bool, _ did: UInt8, _ cid: UInt8, _ payload: [UInt8] = [], quiet: Bool = false) {
        guard link == .ready else { return }
        seq &+= 1
        let data = SpheroPacket(routing: routing, toST: st, did: did, cid: cid, seq: seq, payload: payload).encoded()
        writeAPI(data)
        if !quiet { note("→ \(data.hex)") }
    }

    private func send1(_ did: UInt8, _ cid: UInt8, _ payload: [UInt8] = [], quiet: Bool = false) {
        guard link == .ready else { return }
        seq &+= 1
        let data = V1.encode(did: did, cid: cid, seq: seq, payload: payload)
        writeAPI(data)
        if !quiet { note("→ \(data.hex)") }
    }

    // MARK: - Energía y estado

    func wake() { if kind.usesV2 { send(st: false, Device.power, 0x0D) } }

    func sleep(deep: Bool = false) {
        if kind.usesV2 { send(st: false, Device.power, deep ? 0x00 : 0x01) }
        else { send1(0x00, 0x22, [0, 0, 0, 0, 0]) }
    }

    /// Pide la intensidad de señal; el resultado llega a `rssi`.
    func readRSSI() { if link == .ready { peripheral.readRSSI() } }

    func requestBattery() {
        if kind.usesV2 {
            send(st: false, Device.power, 0x04)
            send(st: false, Device.power, 0x03)
        } else {
            send1(0x00, 0x20, quiet: true)
            v1BatterySeq = seq
        }
    }

    func requestVersions() {
        guard kind.usesV2 else { return }
        send(st: false, Device.systemInfo, 0x00)
        send(st: false, Device.systemInfo, 0x17)
    }

    func rename(to newName: String) {
        guard kind.usesV2 else { return }
        send(st: false, Device.peerConnection, 0x03, Array(newName.utf8))
    }

    // MARK: - Conducción

    /// speed 0…255; heading 0…359 relativo al último "fijar adelante".
    func drive(speed: Int, heading: Int, reverse: Bool = false) {
        let s = UInt8(max(0, min(255, speed)))
        let h = Int(((heading % 360) + 360) % 360)
        if kind.usesV2 {
            // El droide rechaza conducir (error 8, ocupado) mientras reproduce una animación: se cancela.
            if busy, s > 0 { stopAnimation() }
            send(st: true, Device.driving, 0x07, [s] + be16(UInt16(h)) + [reverse ? 1 : 0], quiet: true)
        } else {
            send1(0x02, 0x30, [s] + be16(UInt16(h)) + [s == 0 ? 0 : 1, reverse ? 1 : 0], quiet: true)
        }
    }

    func stop(heading: Int = 0) { drive(speed: 0, heading: heading) }

    /// Gira el droide hacia `heading` sin avanzar (para apuntar).
    func aim(heading: Int) {
        if kind.usesV2 { drive(speed: 0, heading: heading) }
        else { send1(0x02, 0x30, [0] + be16(UInt16(((heading % 360) + 360) % 360)) + [1, 0], quiet: true) }
    }

    /// Define la orientación actual como "adelante".
    func setForward() {
        if kind.usesV2 { send(st: true, Device.driving, 0x06) }
        else { send1(0x02, 0x01, be16(UInt16(0))) }
    }

    func setStabilization(_ on: Bool) {
        if kind.usesV2 { send(st: true, Device.driving, 0x0C, [on ? 1 : 0]) }
        else { send1(0x02, 0x02, [on ? 1 : 0]) }
    }

    /// Turbo de BB-8 (cid 0x31): `time` en décimas de segundo.
    func boostV1(heading: Int, time: UInt8 = 20) {
        if !kind.usesV2 { send1(0x02, 0x31, [time] + be16(UInt16(heading))) }
    }

    // MARK: - Cúpula y patas (R2-D2 / R2-Q5)

    func setDome(_ degrees: Float) { if kind.hasDome { send(st: true, Device.animatronics, 0x0F, beFloat(degrees), quiet: true) } }
    func legAction(_ a: LegAction) { if kind.hasLegs { send(st: true, Device.animatronics, 0x0D, [a.rawValue]) } }
    func setHeadTracking(_ on: Bool) {
        headTracking = on
        if kind.hasDome { send(st: true, Device.animatronics, 0x29, [on ? 1 : 0]) }
    }

    // MARK: - Animaciones y sonido

    func play(_ a: DroidAnimation) {
        guard kind.usesV2 else { return }
        send(st: true, Device.animatronics, 0x05, be16(a.index))
        busy = true
        busyWork?.cancel()
        let w = DispatchWorkItem { [weak self] in self?.busy = false }
        busyWork = w
        DispatchQueue.main.asyncAfter(deadline: .now() + a.seconds + 0.5, execute: w)
    }

    func stopAnimation() {
        guard kind.usesV2 else { return }
        send(st: true, Device.animatronics, 0x2B)
        busy = false
        busyWork?.cancel()
    }

    func play(_ s: DroidSound) { if kind.hasAudio { send(st: true, Device.userIO, 0x07, be16(s.sector) + [0]) } }
    func stopAudio() { if kind.hasAudio { send(st: true, Device.userIO, 0x0A) } }
    func setVolume(_ v: UInt8) { if kind.hasAudio { send(st: true, Device.userIO, 0x08, [v]) } }

    func setChatter(_ on: Bool) {
        chatter = on
        chatterTimer?.invalidate()
        guard on else { return }
        scheduleChatter()
    }

    private func scheduleChatter() {
        let pool = kind.sounds.filter { ["Chatty", "Excited", "Positive", "Hey", "Negative", "Sad", "Laugh"].contains($0.group) }
        chatterTimer = Timer.scheduledTimer(withTimeInterval: .random(in: 6...16), repeats: false) { [weak self] _ in
            guard let self, self.chatter, self.isReady else { return }
            if let s = pool.randomElement() { self.play(s) }
            self.scheduleChatter()
        }
    }

    func setIdleAnimations(_ on: Bool) { idleAnimations = on; if kind.usesV2 { send(st: true, Device.animatronics, 0x2C, [on ? 1 : 0]) } }
    func setTrophyMode(_ on: Bool) { trophyMode = on; if kind.usesV2 { send(st: true, Device.animatronics, 0x2D, [on ? 1 : 0]) } }
    func setDroidToDroid(_ on: Bool) { if kind.usesV2 { send(st: true, Device.animatronics, 0x2F, [on ? 1 : 0]) } }

    func setInteractions(_ m: InteractionMode) {
        setIdleAnimations(m != .off)
        setDroidToDroid(m == .social)
    }

    // MARK: - Luces

    func setLight(_ ch: LightChannel, r: UInt8 = 0, g: UInt8 = 0, b: UInt8 = 0, level: UInt8 = 0) {
        switch ch.style {
        case let .rgb(mr, mg, mb): setV2LEDs(mask: mr | mg | mb, values: [r, g, b])
        case let .mono(m): setV2LEDs(mask: m, values: [level])
        case .v1Main: send1(0x02, 0x20, [r, g, b, 0], quiet: true)
        case .v1Back: send1(0x02, 0x21, [level])
        }
    }

    private func setV2LEDs(mask: UInt16, values: [UInt8]) {
        send(st: true, Device.userIO, 0x0E, be16(mask) + values, quiet: true)
    }

    func setAimLight(_ on: Bool) {
        switch kind {
        case .bb8: send1(0x02, 0x21, [on ? 255 : 0])
        case .bb9e: setV2LEDs(mask: 0x08, values: [on ? 255 : 0])
        default: break
        }
    }

    func resetLights() { if kind.usesV2 { send(st: true, Device.userIO, 0x19) } }

    // MARK: - Colisiones

    func configureCollisions(sensitivity: UInt8 = 70) {
        let p: [UInt8] = [1, sensitivity, sensitivity, sensitivity, sensitivity, 40]
        if kind.usesV2 {
            send(st: true, Device.sensors, 0x11, p)
            send(st: true, Device.sensors, 0x14, [1])
        } else {
            send1(0x02, 0x12, p)
        }
    }

    private func handleHit() {
        note("¡Colisión!")
        guard reactToHits else { return }
        let hits = kind.sounds.filter { $0.group == "Hit" || $0.group == "Negative" }
        if let s = hits.randomElement() { play(s) }
    }

    // MARK: - Respuestas

    private func handle(_ p: ParsedV2) {
        if p.isResponse, let cand = probes[p.seq] {
            probes[p.seq] = nil
            if p.error == 0, !routingConfirmed {
                routing = cand; routingConfirmed = true
                routingText = cand.title
                note("✓ direcciones detectadas: \(cand.title)")
                if let d = try? JSONEncoder().encode(cand) { UserDefaults.standard.set(d, forKey: "routing.\(kind.rawValue)") }
                afterRoutingConfirmed()
            } else {
                note("probe \(cand.title) → error \(p.error ?? 0)")
            }
            return
        }
        if p.isResponse, let e = p.error, e != 0 { noteError(e, did: p.did, cid: p.cid) }

        switch (p.did, p.cid) {
        case (Device.power, 0x04), (Device.power, 0x06):
            if let s = p.payload.first { setBattery(v2State: s) }
        case (Device.power, 0x03):
            if let mv = readU16(p.payload, 0) { batteryText = String(format: "%.2f V", Double(mv) / 100) }
        case (Device.systemInfo, 0x00):
            versions["Nordic"] = version(p.payload)
        case (Device.systemInfo, 0x17), (Device.systemInfo, 0x18):
            versions["Principal"] = version(p.payload)
        case (Device.sensors, 0x12):
            collision.send()
        case (Device.animatronics, 0x11):
            busy = false
        default:
            // Los acuses correctos y vacíos (uno por cada orden de conducción) solo meterían ruido.
            if p.isResponse, p.error == 0, p.payload.isEmpty { break }
            note("← \(String(format: "%02X/%02X", p.did, p.cid)) \(p.payload.hex)")
        }
    }

    private func version(_ b: [UInt8]) -> String {
        guard let a = readU16(b, 0), let m = readU16(b, 2), let r = readU16(b, 4) else { return b.hex }
        return "\(a).\(m).\(r)"
    }

    private func setBattery(v2State s: UInt8) {
        batteryLevel = [0: .charged, 1: .charging, 2: .ok, 3: .ok, 4: .low, 5: .critical][s] ?? .unknown
    }

    private func handle(_ p: ParsedV1) {
        if p.isAsync {
            switch p.code {
            case 0x07: collision.send()
            case 0x01: if p.payload.count >= 1 { setBattery(v1State: p.payload[0]) }
            default: note("← async \(String(format: "%02X", p.code)) \(p.payload.hex)")
            }
            return
        }
        if p.code != 0 { note("✗ error \(p.code)") }
        // Respuesta de GetPowerState: [versión, estado, voltaje(2), cargas(2), tiempo(2)]
        if p.seq == v1BatterySeq, p.payload.count >= 4 {
            setBattery(v1State: p.payload[1])
            if let mv = readU16(p.payload, 2) { batteryText = String(format: "%.2f V", Double(mv) / 100) }
        }
    }

    private func setBattery(v1State s: UInt8) {
        batteryLevel = [1: .charging, 2: .ok, 3: .low, 4: .critical][s] ?? .unknown
    }
}

// MARK: - CoreBluetooth

extension Droid: CBPeripheralDelegate {
    func peripheral(_ p: CBPeripheral, didReadRSSI RSSI: NSNumber, error: Error?) {
        if error == nil, RSSI.intValue != 127 { rssi = RSSI.intValue }   // 127 = no disponible
    }

    func peripheral(_ p: CBPeripheral, didDiscoverServices error: Error?) {
        let services = p.services ?? []
        pendingServices = services.count
        for s in services { note("servicio \(s.uuid)"); p.discoverCharacteristics(nil, for: s) }
    }

    func peripheral(_ p: CBPeripheral, didDiscoverCharacteristicsFor s: CBService, error: Error?) {
        for c in s.characteristics ?? [] {
            note("  característica \(c.uuid) props=\(c.properties.rawValue)")
            chars[c.uuid] = c
        }
        pendingServices -= 1
        guard pendingServices == 0 else { return }
        beginSetup()
    }

    private func beginSetup() {
        status = "Preparando conexión…"
        if kind.usesV2 {
            guard let api = chars[Self.v2Api], let anti = chars[Self.v2AntiDos] else {
                status = "No es un droide compatible (faltan características)"; note(status); return
            }
            peripheral.setNotifyValue(true, for: api)
            peripheral.writeValue(Data(Self.v2Secret.utf8), for: anti, type: .withResponse)
        } else {
            guard let resp = chars[Self.v1Response], let anti = chars[Self.v1AntiDos], chars[Self.v1Command] != nil else {
                status = "No es un droide compatible (faltan características)"; note(status); return
            }
            peripheral.setNotifyValue(true, for: resp)
            peripheral.writeValue(Data(Self.v1Secret.utf8), for: anti, type: .withResponse)
        }
    }

    func peripheral(_ p: CBPeripheral, didWriteValueFor c: CBCharacteristic, error: Error?) {
        if let error { status = "Error de escritura: \(error.localizedDescription)"; note(status); return }
        if kind.usesV2 {
            if c.uuid == Self.v2AntiDos { becameReady() }
            return
        }
        // BB-8: anti-DoS → potencia de transmisión → despertar CPU
        switch c.uuid {
        case Self.v1AntiDos:
            if let tx = chars[Self.v1TxPower] { p.writeValue(Data([0x07]), for: tx, type: .withResponse) }
        case Self.v1TxPower:
            if let w = chars[Self.v1Wake] { p.writeValue(Data([0x01]), for: w, type: .withResponse) }
        case Self.v1Wake:
            becameReady()
        default: break
        }
    }

    func peripheral(_ p: CBPeripheral, didUpdateValueFor c: CBCharacteristic, error: Error?) {
        guard let data = c.value else { return }
        if kind.usesV2, c.uuid == Self.v2Api {
            for pkt in v2Decoder.feed(data) { handle(pkt) }
        } else if !kind.usesV2, c.uuid == Self.v1Response {
            for pkt in v1Decoder.feed(data) { handle(pkt) }
        }
    }
}
