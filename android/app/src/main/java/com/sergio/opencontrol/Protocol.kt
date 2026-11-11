package com.sergio.opencontrol

// IDs verificados en el APK de la app original (ver tools/sdk_commands.txt). Mismo protocolo que
// ios/Sources/SpheroPacket.swift y desktop/src/protocol.rs.

/// Dispositivos de la API v2 (R2-D2, R2-Q5, BB-9E).
object Dev {
    const val API_PROCESSOR = 0x10
    const val SYSTEM_INFO = 0x11
    const val POWER = 0x13
    const val DRIVING = 0x16
    const val ANIMATRONICS = 0x17
    const val SENSORS = 0x18
    const val PEER_CONNECTION = 0x19
    const val USER_IO = 0x1A
}

/// Cómo se direccionan los dos procesadores del droide. La app prueba las variantes al conectar.
data class Routing(val nordic: Int, val st: Int, val withSource: Boolean) {
    val title: String get() = "dest %02X/%02X%s".format(nordic, st, if (withSource) " +src" else "")

    companion object {
        /// El primero es el confirmado en un R2-D2 real; el resto son alternativas.
        val CANDIDATES = listOf(
            Routing(0x01, 0x02, false),
            Routing(0x11, 0x12, true),
            Routing(0x01, 0x02, true),
            Routing(0x11, 0x12, false),
        )
    }
}

private const val SOP = 0x8D
private const val EOP = 0xD8
private const val ESC = 0xAB

fun encodeV2(routing: Routing, toST: Boolean, did: Int, cid: Int, seq: Int, payload: List<Int> = emptyList()): ByteArray {
    // Flags: 0x02 pide respuesta, 0x10 lleva target, 0x20 lleva source.
    var flags = 0x02 or 0x10
    if (routing.withSource) flags = flags or 0x20
    val body = mutableListOf(flags, if (toST) routing.st else routing.nordic)
    if (routing.withSource) body.add(0x01)
    body.addAll(listOf(did, cid, seq and 0xFF))
    body.addAll(payload.map { it and 0xFF })
    body.add(body.sum().inv() and 0xFF)

    val out = mutableListOf(SOP)
    for (b in body) {
        when (b) {
            SOP -> out.addAll(listOf(ESC, 0x05))
            ESC -> out.addAll(listOf(ESC, 0x23))
            EOP -> out.addAll(listOf(ESC, 0x50))
            else -> out.add(b)
        }
    }
    out.add(EOP)
    return out.toBytes()
}

data class ParsedV2(val isResponse: Boolean, val did: Int, val cid: Int, val seq: Int, val error: Int?, val payload: List<Int>)

/// Reensambla paquetes v2 desde los fragmentos de notificación BLE.
class DecoderV2 {
    private var buffer = mutableListOf<Int>()

    fun feed(data: ByteArray): List<ParsedV2> {
        val out = mutableListOf<ParsedV2>()
        for (raw in data) {
            val b = raw.toInt() and 0xFF
            when (b) {
                SOP -> buffer = mutableListOf()
                EOP -> {
                    parse(unescape(buffer))?.let(out::add)
                    buffer = mutableListOf()
                }
                else -> buffer.add(b)
            }
        }
        return out
    }

    private fun unescape(raw: List<Int>): List<Int> {
        val out = mutableListOf<Int>()
        var escaped = false
        for (b in raw) {
            if (escaped) {
                out.add(when (b) { 0x05 -> SOP; 0x23 -> ESC; 0x50 -> EOP; else -> b })
                escaped = false
            } else if (b == ESC) {
                escaped = true
            } else {
                out.add(b)
            }
        }
        return out
    }

    private fun parse(b: List<Int>): ParsedV2? {
        if (b.size < 5) return null
        val flags = b[0]
        var i = 1
        if (flags and 0x10 != 0) i += 1
        if (flags and 0x20 != 0) i += 1
        if (b.size < i + 4) return null
        val (did, cid, seq) = Triple(b[i], b[i + 1], b[i + 2])
        val body = b.subList(i + 3, b.size - 1) // sin checksum
        return if (flags and 0x01 != 0) {
            ParsedV2(true, did, cid, seq, body.firstOrNull() ?: 0, body.drop(1))
        } else {
            ParsedV2(false, did, cid, seq, null, body.toList())
        }
    }
}

// ---------------------------------------------------------------- API v1 (BB-8)

fun encodeV1(did: Int, cid: Int, seq: Int, payload: List<Int> = emptyList()): ByteArray {
    val dlen = payload.size + 1
    val sum = (did + cid + (seq and 0xFF) + dlen + payload.sumOf { it and 0xFF }) and 0xFF
    return (listOf(0xFF, 0xFF, did, cid, seq and 0xFF, dlen) + payload.map { it and 0xFF } + listOf(sum.inv() and 0xFF)).toBytes()
}

/// `code` es mrsp en respuestas y el id de async en notificaciones.
data class ParsedV1(val isAsync: Boolean, val code: Int, val seq: Int, val payload: List<Int>)

class DecoderV1 {
    private val buf = ArrayList<Int>()

    fun feed(data: ByteArray): List<ParsedV1> {
        data.forEach { buf.add(it.toInt() and 0xFF) }
        val out = mutableListOf<ParsedV1>()
        while (buf.size >= 5) {
            if (buf[0] != 0xFF) {
                buf.removeAt(0)
                continue
            }
            if (buf[1] == 0xFF) {
                val len = buf[4] + 5
                if (buf.size < len) break
                out.add(ParsedV1(false, buf[2], buf[3], buf.subList(5, len - 1).toList()))
                repeat(len) { buf.removeAt(0) }
            } else if (buf[1] == 0xFE) {
                val len = ((buf[3] shl 8) or buf[4]) + 5
                if (len > 2048) {
                    buf.clear()
                    break
                }
                if (buf.size < len) break
                out.add(ParsedV1(true, buf[2], 0, buf.subList(5, len - 1).toList()))
                repeat(len) { buf.removeAt(0) }
            } else {
                buf.removeAt(0)
            }
        }
        return out
    }
}

// ---------------------------------------------------------------- utilidades

fun List<Int>.toBytes(): ByteArray = ByteArray(size) { this[it].toByte() }
fun ByteArray.hex(): String = joinToString(" ") { "%02X".format(it.toInt() and 0xFF) }
fun List<Int>.hex(): String = joinToString(" ") { "%02X".format(it and 0xFF) }
fun be16(v: Int): List<Int> = listOf((v shr 8) and 0xFF, v and 0xFF)
fun beFloat(v: Float): List<Int> = v.toRawBits().let { listOf(it ushr 24 and 0xFF, it ushr 16 and 0xFF, it ushr 8 and 0xFF, it and 0xFF) }
fun readU16(b: List<Int>, at: Int): Int? = if (b.size >= at + 2) (b[at] shl 8) or b[at + 1] else null
