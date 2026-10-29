//! Paquetes de la API v2 (R2-D2, R2-Q5, BB-9E) y v1 (BB-8) de Sphero.
//! IDs y formatos verificados en el APK de la app oficial; ver docs/PROTOCOLO.md.

pub const SOP: u8 = 0x8D;
pub const EOP: u8 = 0xD8;
pub const ESC: u8 = 0xAB;

/// Dispositivos de la API v2.
pub mod dev {
    pub const API_PROCESSOR: u8 = 0x10;
    pub const SYSTEM_INFO: u8 = 0x11;
    pub const POWER: u8 = 0x13;
    pub const DRIVING: u8 = 0x16;
    pub const ANIMATRONICS: u8 = 0x17;
    pub const SENSORS: u8 = 0x18;
    pub const PEER_CONNECTION: u8 = 0x19;
    pub const USER_IO: u8 = 0x1A;
}

/// Cómo se direccionan los dos procesadores del droide (Nordic = Bluetooth, ST = movimiento y audio).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Routing {
    pub nordic: u8,
    pub st: u8,
    pub with_source: bool,
}

impl Routing {
    /// El primero está confirmado en un R2-D2 real; el resto son alternativas que se prueban al conectar.
    pub const CANDIDATES: [Routing; 4] = [
        Routing { nordic: 0x01, st: 0x02, with_source: false },
        Routing { nordic: 0x11, st: 0x12, with_source: true },
        Routing { nordic: 0x01, st: 0x02, with_source: true },
        Routing { nordic: 0x11, st: 0x12, with_source: false },
    ];

    pub fn title(&self) -> String {
        format!("dest {:02X}/{:02X}{}", self.nordic, self.st, if self.with_source { " +src" } else { "" })
    }
}

pub fn encode_v2(routing: Routing, to_st: bool, did: u8, cid: u8, seq: u8, payload: &[u8]) -> Vec<u8> {
    // Flags: 0x02 pide respuesta, 0x10 lleva target, 0x20 lleva source.
    let mut flags = 0x02 | 0x10;
    if routing.with_source {
        flags |= 0x20;
    }
    let mut body = vec![flags, if to_st { routing.st } else { routing.nordic }];
    if routing.with_source {
        body.push(0x01);
    }
    body.extend_from_slice(&[did, cid, seq]);
    body.extend_from_slice(payload);
    let sum = body.iter().fold(0u8, |a, b| a.wrapping_add(*b));
    body.push(!sum);

    let mut out = vec![SOP];
    for b in body {
        match b {
            SOP => out.extend_from_slice(&[ESC, 0x05]),
            ESC => out.extend_from_slice(&[ESC, 0x23]),
            EOP => out.extend_from_slice(&[ESC, 0x50]),
            _ => out.push(b),
        }
    }
    out.push(EOP);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedV2 {
    pub is_response: bool,
    pub did: u8,
    pub cid: u8,
    pub seq: u8,
    /// Solo en respuestas; 0 = correcto.
    pub error: Option<u8>,
    pub payload: Vec<u8>,
}

/// Reensambla paquetes v2 desde los fragmentos de notificación BLE.
#[derive(Default)]
pub struct DecoderV2 {
    buf: Vec<u8>,
}

impl DecoderV2 {
    pub fn feed(&mut self, data: &[u8]) -> Vec<ParsedV2> {
        let mut out = Vec::new();
        for &b in data {
            match b {
                SOP => self.buf.clear(),
                EOP => {
                    if let Some(p) = parse_v2(&unescape(&self.buf)) {
                        out.push(p);
                    }
                    self.buf.clear();
                }
                _ => self.buf.push(b),
            }
        }
        out
    }
}

fn unescape(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut escaped = false;
    for &b in raw {
        if escaped {
            out.push(match b {
                0x05 => SOP,
                0x23 => ESC,
                0x50 => EOP,
                other => other,
            });
            escaped = false;
        } else if b == ESC {
            escaped = true;
        } else {
            out.push(b);
        }
    }
    out
}

fn parse_v2(b: &[u8]) -> Option<ParsedV2> {
    if b.len() < 5 {
        return None;
    }
    let flags = b[0];
    let mut i = 1;
    if flags & 0x10 != 0 {
        i += 1;
    }
    if flags & 0x20 != 0 {
        i += 1;
    }
    if b.len() < i + 4 {
        return None;
    }
    let (did, cid, seq) = (b[i], b[i + 1], b[i + 2]);
    let body = &b[i + 3..b.len() - 1]; // sin checksum
    if flags & 0x01 != 0 {
        Some(ParsedV2 {
            is_response: true,
            did,
            cid,
            seq,
            error: Some(body.first().copied().unwrap_or(0)),
            payload: body.iter().skip(1).copied().collect(),
        })
    } else {
        Some(ParsedV2 { is_response: false, did, cid, seq, error: None, payload: body.to_vec() })
    }
}

// ---------------------------------------------------------------- API v1 (BB-8)

pub fn encode_v1(did: u8, cid: u8, seq: u8, payload: &[u8]) -> Vec<u8> {
    let dlen = (payload.len() + 1) as u8;
    let sum = [did, cid, seq, dlen]
        .iter()
        .chain(payload.iter())
        .fold(0u8, |a, b| a.wrapping_add(*b));
    let mut out = vec![0xFF, 0xFF, did, cid, seq, dlen];
    out.extend_from_slice(payload);
    out.push(!sum);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedV1 {
    pub is_async: bool,
    /// `mrsp` en respuestas, id del evento en notificaciones.
    pub code: u8,
    pub seq: u8,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct DecoderV1 {
    buf: Vec<u8>,
}

impl DecoderV1 {
    pub fn feed(&mut self, data: &[u8]) -> Vec<ParsedV1> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        while self.buf.len() >= 5 {
            if self.buf[0] != 0xFF {
                self.buf.remove(0);
                continue;
            }
            match self.buf[1] {
                0xFF => {
                    let len = self.buf[4] as usize + 5;
                    if self.buf.len() < len {
                        break;
                    }
                    out.push(ParsedV1 {
                        is_async: false,
                        code: self.buf[2],
                        seq: self.buf[3],
                        payload: self.buf[5..len - 1].to_vec(),
                    });
                    self.buf.drain(..len);
                }
                0xFE => {
                    let len = ((self.buf[3] as usize) << 8 | self.buf[4] as usize) + 5;
                    if len > 2048 {
                        self.buf.clear();
                        break;
                    }
                    if self.buf.len() < len {
                        break;
                    }
                    out.push(ParsedV1 { is_async: true, code: self.buf[2], seq: 0, payload: self.buf[5..len - 1].to_vec() });
                    self.buf.drain(..len);
                }
                _ => {
                    self.buf.remove(0);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- utilidades

pub fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

pub fn be_float(v: f32) -> [u8; 4] {
    v.to_be_bytes()
}

pub fn read_u16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: Routing = Routing { nordic: 0x11, st: 0x12, with_source: true };

    #[test]
    fn v2_wake_matches_known_vector() {
        // Mismo vector que la app de iOS.
        assert_eq!(hex(&encode_v2(OLD, false, dev::POWER, 0x0D, 1, &[])), "8D 32 11 01 13 0D 01 9A D8");
    }

    #[test]
    fn v2_confirmed_routing_matches_real_r2d2_capture() {
        // Capturado de un R2-D2 real: 8D 12 02 16 06 5F 70 D8
        let r = Routing::CANDIDATES[0];
        assert_eq!(hex(&encode_v2(r, true, dev::DRIVING, 0x06, 0x5F, &[])), "8D 12 02 16 06 5F 70 D8");
        // y la consulta de batería al Nordic: 8D 12 01 13 04 60 75 D8
        assert_eq!(hex(&encode_v2(r, false, dev::POWER, 0x04, 0x60, &[])), "8D 12 01 13 04 60 75 D8");
    }

    #[test]
    fn v2_escaping_round_trips() {
        let enc = encode_v2(OLD, true, 0x17, 0x0F, 2, &[0x8D, 0xAB, 0xD8]);
        assert!(hex(&enc).contains("AB 05 AB 23 AB 50"));
        let parsed = DecoderV2::default().feed(&enc);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].payload, vec![0x8D, 0xAB, 0xD8]);
    }

    #[test]
    fn v2_decodes_response_even_when_split_in_fragments() {
        let mut body = vec![0x01, 0x10, 0x00, 0x07, 0x00];
        let sum = body.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        body.push(!sum);
        let mut raw = vec![SOP];
        raw.extend(body);
        raw.push(EOP);
        let mut d = DecoderV2::default();
        assert!(d.feed(&raw[..3]).is_empty());
        let p = d.feed(&raw[3..]);
        assert_eq!(p.len(), 1);
        assert!(p[0].is_response);
        assert_eq!((p[0].did, p[0].cid, p[0].seq, p[0].error), (0x10, 0x00, 0x07, Some(0)));
    }

    #[test]
    fn v1_roll_and_decoders() {
        assert_eq!(hex(&encode_v1(0x02, 0x30, 3, &[100, 0x00, 0x5A, 1, 0])), "FF FF 02 30 03 06 64 00 5A 01 00 05");
        let mut d = DecoderV1::default();
        let r = d.feed(&[0xFF, 0xFF, 0x00, 0x03, 0x01, 0xFB]);
        assert_eq!((r[0].is_async, r[0].code, r[0].seq), (false, 0, 3));
        let a = d.feed(&[0xFF, 0xFE, 0x07, 0x00, 0x03, 1, 2, 0xF0]);
        assert_eq!((a[0].is_async, a[0].code, a[0].payload.clone()), (true, 7, vec![1, 2]));
    }
}
