import Foundation

// MARK: - IDs verificados en el APK (Assembly-CSharp-firstpass.dll / libCore.so)

/// Dispositivos de la API v2 (R2-D2, R2-Q5, BB-9E).
enum Device {
    static let apiProcessor: UInt8 = 0x10
    static let systemInfo: UInt8 = 0x11
    static let power: UInt8 = 0x13
    static let driving: UInt8 = 0x16
    static let animatronics: UInt8 = 0x17
    static let sensors: UInt8 = 0x18
    static let peerConnection: UInt8 = 0x19
    static let userIO: UInt8 = 0x1A
}

/// Cómo se direccionan los dos procesadores del droide. No se pudo confirmar desde el APK
/// (lo construye código nativo), así que la app prueba las variantes al conectar.
struct Routing: Equatable, Codable {
    var nordic: UInt8
    var st: UInt8
    var withSource: Bool

    static let candidates: [Routing] = [
        Routing(nordic: 0x11, st: 0x12, withSource: true),
        Routing(nordic: 0x01, st: 0x02, withSource: true),
        Routing(nordic: 0x01, st: 0x02, withSource: false),
        Routing(nordic: 0x11, st: 0x12, withSource: false),
    ]
    var title: String { String(format: "dest %02X/%02X%@", nordic, st, withSource ? " +src" : "") }
}

// MARK: - API v2

struct SpheroPacket {
    static let sop: UInt8 = 0x8D
    static let eop: UInt8 = 0xD8
    static let esc: UInt8 = 0xAB

    var routing: Routing
    var toST: Bool
    var did: UInt8
    var cid: UInt8
    var seq: UInt8
    var payload: [UInt8] = []

    func encoded() -> Data {
        // Flags: 0x02 pide respuesta, 0x10 lleva target, 0x20 lleva source.
        var flags: UInt8 = 0x02 | 0x10
        if routing.withSource { flags |= 0x20 }
        var body: [UInt8] = [flags, toST ? routing.st : routing.nordic]
        if routing.withSource { body.append(0x01) }
        body += [did, cid, seq] + payload
        body.append(~body.reduce(0) { $0 &+ $1 })

        var out: [UInt8] = [Self.sop]
        for b in body {
            switch b {
            case Self.sop: out += [Self.esc, 0x05]
            case Self.esc: out += [Self.esc, 0x23]
            case Self.eop: out += [Self.esc, 0x50]
            default: out.append(b)
            }
        }
        out.append(Self.eop)
        return Data(out)
    }
}

struct ParsedV2 {
    var isResponse: Bool
    var did: UInt8
    var cid: UInt8
    var seq: UInt8
    var error: UInt8?   // solo en respuestas; 0 = correcto
    var payload: [UInt8]
}

/// Reensambla paquetes v2 desde los fragmentos de notificación BLE.
struct PacketDecoder {
    private var buffer: [UInt8] = []

    mutating func feed(_ data: Data) -> [ParsedV2] {
        var out: [ParsedV2] = []
        for b in data {
            if b == SpheroPacket.sop { buffer = []; continue }
            if b == SpheroPacket.eop {
                if let p = parse(unescape(buffer)) { out.append(p) }
                buffer = []
                continue
            }
            buffer.append(b)
        }
        return out
    }

    private func unescape(_ raw: [UInt8]) -> [UInt8] {
        var out: [UInt8] = []
        var escaped = false
        for b in raw {
            if escaped {
                switch b {
                case 0x05: out.append(SpheroPacket.sop)
                case 0x23: out.append(SpheroPacket.esc)
                case 0x50: out.append(SpheroPacket.eop)
                default: out.append(b)
                }
                escaped = false
            } else if b == SpheroPacket.esc {
                escaped = true
            } else {
                out.append(b)
            }
        }
        return out
    }

    private func parse(_ b: [UInt8]) -> ParsedV2? {
        guard b.count >= 5 else { return nil }
        let flags = b[0]
        var i = 1
        if flags & 0x10 != 0 { i += 1 }
        if flags & 0x20 != 0 { i += 1 }
        guard b.count >= i + 4 else { return nil }
        let did = b[i], cid = b[i + 1], seq = b[i + 2]
        let body = Array(b[(i + 3)..<(b.count - 1)]) // sin checksum
        if flags & 0x01 != 0 {
            return ParsedV2(isResponse: true, did: did, cid: cid, seq: seq, error: body.first ?? 0,
                            payload: Array(body.dropFirst()))
        }
        return ParsedV2(isResponse: false, did: did, cid: cid, seq: seq, error: nil, payload: body)
    }
}

// MARK: - API v1 (BB-8)

enum V1 {
    static let sop1: UInt8 = 0xFF
    static let sop2Response: UInt8 = 0xFF
    static let sop2Async: UInt8 = 0xFE

    static func encode(did: UInt8, cid: UInt8, seq: UInt8, payload: [UInt8]) -> Data {
        let dlen = UInt8(payload.count + 1)
        let sum = [did, cid, seq, dlen].reduce(0) { $0 &+ $1 } &+ payload.reduce(0) { $0 &+ $1 }
        return Data([sop1, sop2Response, did, cid, seq, dlen] + payload + [~sum])
    }
}

struct ParsedV1 {
    var isAsync: Bool
    var code: UInt8      // mrsp en respuestas, id de async en notificaciones
    var seq: UInt8
    var payload: [UInt8]
}

struct V1Decoder {
    private var buf: [UInt8] = []

    mutating func feed(_ data: Data) -> [ParsedV1] {
        buf += data
        var out: [ParsedV1] = []
        while buf.count >= 5 {
            guard buf[0] == V1.sop1 else { buf.removeFirst(); continue }
            if buf[1] == V1.sop2Response {
                let len = Int(buf[4]) + 5
                guard buf.count >= len else { break }
                out.append(ParsedV1(isAsync: false, code: buf[2], seq: buf[3], payload: Array(buf[5..<(len - 1)])))
                buf.removeFirst(len)
            } else if buf[1] == V1.sop2Async {
                let len = (Int(buf[3]) << 8 | Int(buf[4])) + 5
                guard len <= 2048 else { buf = []; break }
                guard buf.count >= len else { break }
                out.append(ParsedV1(isAsync: true, code: buf[2], seq: 0, payload: Array(buf[5..<(len - 1)])))
                buf.removeFirst(len)
            } else {
                buf.removeFirst()
            }
        }
        return out
    }
}

// MARK: - Utilidades

extension Data {
    var hex: String { map { String(format: "%02X", $0) }.joined(separator: " ") }
}

extension Array where Element == UInt8 {
    var hex: String { map { String(format: "%02X", $0) }.joined(separator: " ") }
}

func be16(_ v: UInt16) -> [UInt8] { [UInt8(v >> 8), UInt8(v & 0xFF)] }
func be16(_ v: Int16) -> [UInt8] { be16(UInt16(bitPattern: v)) }
func beFloat(_ v: Float) -> [UInt8] { withUnsafeBytes(of: v.bitPattern.bigEndian) { Array($0) } }
func readU16(_ b: [UInt8], _ at: Int) -> UInt16? {
    guard b.count >= at + 2 else { return nil }
    return UInt16(b[at]) << 8 | UInt16(b[at + 1])
}
