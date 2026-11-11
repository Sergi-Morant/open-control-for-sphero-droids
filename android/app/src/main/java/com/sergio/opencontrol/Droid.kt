package com.sergio.opencontrol

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlin.random.Random

enum class Link { CONNECTING, READY, DISCONNECTED }

enum class Battery(val text: String) {
    UNKNOWN("—"), CHARGED("Cargado"), CHARGING("Cargando"), OK("Normal"), LOW("Baja"), CRITICAL("Crítica")
}

enum class LegAction(val code: Int, val title: String) {
    STOP(0, "Parar"), THREE_LEGS(1, "3 patas"), TWO_LEGS(2, "2 patas"), WADDLE(3, "Contoneo")
}

enum class Interactions(val title: String) {
    OFF("Ninguna"), IDLE("Animaciones de reposo"), SOCIAL("Reposo + otros droides")
}

/// Un droide concreto: estado, protocolo (v1 o v2) y órdenes de alto nivel. No sabe nada de Bluetooth:
/// escribe por `write` y recibe por `onNotification`, así se puede probar sin teléfono.
/// Los tiempos (`now`) son milisegundos de un reloj monótono. Todo se usa desde el hilo principal.
class Droid(val id: String, val name: String, val kind: Kind, private val write: (ByteArray) -> Unit) {
    var link by mutableStateOf(Link.CONNECTING)
    var status by mutableStateOf("Conectando…")
    val log = mutableStateListOf<String>()
    var battery by mutableStateOf(Battery.UNKNOWN)
    var batteryText by mutableStateOf("—")
    val versions = mutableStateMapOf<String, String>()
    var routingText by mutableStateOf("sin detectar")
    /// Señal Bluetooth medida desde el teléfono (dBm) y cuándo llegó.
    var rssi by mutableStateOf<Int?>(null)
    var rssiAt: Long? = null
    var busy by mutableStateOf(false)
    var chatter by mutableStateOf(false)
    var reactToHits by mutableStateOf(true)
    var trophyMode by mutableStateOf(false)
    var headTracking by mutableStateOf(false)

    val isReady: Boolean get() = link == Link.READY

    private var busyUntil: Long? = null
    private var hitPending = false
    private var routing = Routing.CANDIDATES[0]
    private var routingConfirmed = false
    private val probes = HashMap<Int, Routing>()
    private var probeStart: Long? = null
    private var probeSent = 0
    private var probeWarned = false
    private var seq = 0
    private val decV2 = DecoderV2()
    private val decV1 = DecoderV1()
    private var v1BatterySeq = 0
    private var lastPoll = 0L
    private var nextChatter = 0L
    private var lastError = ""
    private var lastErrorAt = Long.MIN_VALUE
    private var started: Long? = null
    private var clock = 0L

    fun note(s: String) {
        val t = (clock - (started ?: clock)) / 1000.0
        log.add("%8.3f %s".format(t, s))
        if (log.size > 600) log.removeRange(0, 200)
    }

    /// Devuelve true una sola vez por cada colisión detectada.
    fun takeHit(): Boolean = hitPending.also { hitPending = false }

    // ------------------------------------------------------------- ciclo de vida

    fun onReady(now: Long) {
        clock = now
        if (started == null) started = now
        link = Link.READY
        status = "Conectado"
        lastPoll = now
        if (kind.usesV2) {
            wake()
            probeStart = now + 1000
        } else {
            afterRoutingConfirmed()
        }
    }

    fun onDisconnected() {
        link = Link.DISCONNECTED
        status = "Desconectado"
        chatter = false
        busy = false
        busyUntil = null
        routingConfirmed = false
        probeStart = null
        probeSent = 0
        probeWarned = false
    }

    /// Se llama unas diez veces por segundo.
    fun tick(now: Long) {
        clock = now
        if (link != Link.READY) return
        val start = probeStart
        if (kind.usesV2 && !routingConfirmed && start != null) {
            val n = Routing.CANDIDATES.size
            while (probeSent < n && now >= start + 500L * probeSent) {
                sendProbe(probeSent)
                probeSent += 1
            }
            if (probeSent == n && !probeWarned && now >= start + 500L * n + 1500) {
                probeWarned = true
                routingText = "no se pudo detectar"
                note("AVISO: ningún esquema de direcciones obtuvo respuesta del procesador principal")
            }
        }
        if (now - lastPoll >= 30_000) {
            lastPoll = now
            requestBattery()
        }
        if (chatter && now >= nextChatter) {
            val groups = setOf("Chatty", "Excited", "Positive", "Hey", "Negative", "Sad", "Laugh")
            kind.sounds.filter { it.group in groups }.randomOrNull()?.let(::playSound)
            nextChatter = now + Random.nextLong(6_000, 16_000)
        }
        if (busyUntil?.let { now >= it } == true) {
            busyUntil = null
            busy = false
        }
    }

    private fun sendProbe(idx: Int) {
        val cand = Routing.CANDIDATES[idx]
        seq = (seq + 1) and 0xFF
        probes[seq] = cand
        write(encodeV2(cand, true, Dev.API_PROCESSOR, 0x00, seq))
        note("probe ${cand.title}")
    }

    private fun afterRoutingConfirmed() {
        wake()
        configureCollisions()
        if (kind.usesV2) {
            sendV2(false, Dev.POWER, 0x05, listOf(1)) // notificaciones de batería
            requestVersions()
        } else {
            sendV1(0x00, 0x21, listOf(1)) // notificaciones de energía (v1)
        }
        requestBattery()
    }

    // ------------------------------------------------------------- envío

    private fun sendV2(st: Boolean, did: Int, cid: Int, payload: List<Int> = emptyList(), quiet: Boolean = false) {
        if (!isReady) return
        seq = (seq + 1) and 0xFF
        val data = encodeV2(routing, st, did, cid, seq, payload)
        if (!quiet) note("-> ${data.hex()}")
        write(data)
    }

    private fun sendV1(did: Int, cid: Int, payload: List<Int> = emptyList(), quiet: Boolean = false) {
        if (!isReady) return
        seq = (seq + 1) and 0xFF
        val data = encodeV1(did, cid, seq, payload)
        if (!quiet) note("-> ${data.hex()}")
        write(data)
    }

    // ------------------------------------------------------------- energía y estado

    fun wake() {
        if (kind.usesV2) sendV2(false, Dev.POWER, 0x0D)
    }

    fun sleep(deep: Boolean = false) {
        if (kind.usesV2) sendV2(false, Dev.POWER, if (deep) 0x00 else 0x01) else sendV1(0x00, 0x22, listOf(0, 0, 0, 0, 0))
    }

    fun requestBattery() {
        if (kind.usesV2) {
            sendV2(false, Dev.POWER, 0x04)
            sendV2(false, Dev.POWER, 0x03)
        } else {
            sendV1(0x00, 0x20, quiet = true)
            v1BatterySeq = seq
        }
    }

    fun requestVersions() {
        if (!kind.usesV2) return
        sendV2(false, Dev.SYSTEM_INFO, 0x00)
        sendV2(false, Dev.SYSTEM_INFO, 0x17)
    }

    fun rename(newName: String) {
        if (kind.usesV2) sendV2(false, Dev.PEER_CONNECTION, 0x03, newName.toByteArray().map { it.toInt() and 0xFF })
    }

    // ------------------------------------------------------------- conducción

    /// speed 0…255; heading 0…359 relativo al último «fijar adelante».
    fun drive(speed: Int, heading: Int, reverse: Boolean = false) {
        val s = speed.coerceIn(0, 255)
        val h = heading.mod(360)
        if (kind.usesV2) {
            // El droide rechaza conducir (error 8, ocupado) mientras reproduce una animación: se cancela.
            if (busy && s > 0) stopAnimation()
            sendV2(true, Dev.DRIVING, 0x07, listOf(s) + be16(h) + listOf(if (reverse) 1 else 0), quiet = true)
        } else {
            sendV1(0x02, 0x30, listOf(s) + be16(h) + listOf(if (s == 0) 0 else 1, if (reverse) 1 else 0), quiet = true)
        }
    }

    fun stop(heading: Int = 0) = drive(0, heading)

    /// Gira el droide hacia `heading` sin avanzar (para apuntar).
    fun aim(heading: Int) {
        if (kind.usesV2) drive(0, heading) else sendV1(0x02, 0x30, listOf(0) + be16(heading.mod(360)) + listOf(1, 0), quiet = true)
    }

    /// Define la orientación actual como «adelante».
    fun setForward() {
        if (kind.usesV2) sendV2(true, Dev.DRIVING, 0x06) else sendV1(0x02, 0x01, be16(0))
    }

    fun setStabilization(on: Boolean) {
        val v = listOf(if (on) 1 else 0)
        if (kind.usesV2) sendV2(true, Dev.DRIVING, 0x0C, v) else sendV1(0x02, 0x02, v)
    }

    /// Turbo de BB-8 (cid 0x31): `time` en décimas de segundo.
    fun boostV1(heading: Int, time: Int = 20) {
        if (!kind.usesV2) sendV1(0x02, 0x31, listOf(time) + be16(heading.mod(360)))
    }

    // ------------------------------------------------------------- cúpula y patas (R2-D2 / R2-Q5)

    fun setDome(degrees: Float) {
        if (kind.hasDome) sendV2(true, Dev.ANIMATRONICS, 0x0F, beFloat(degrees), quiet = true)
    }

    fun legAction(a: LegAction) {
        if (kind.hasDome) sendV2(true, Dev.ANIMATRONICS, 0x0D, listOf(a.code))
    }

    fun setHeadTrackingOn(on: Boolean) {
        headTracking = on
        if (kind.hasDome) sendV2(true, Dev.ANIMATRONICS, 0x29, listOf(if (on) 1 else 0))
    }

    // ------------------------------------------------------------- animaciones y sonido

    fun playAnimation(a: Animation, now: Long) {
        if (!kind.usesV2) return
        sendV2(true, Dev.ANIMATRONICS, 0x05, be16(a.index))
        busy = true
        busyUntil = now + ((a.seconds + 0.5f) * 1000).toLong()
    }

    fun stopAnimation() {
        if (!kind.usesV2) return
        sendV2(true, Dev.ANIMATRONICS, 0x2B)
        busy = false
        busyUntil = null
    }

    fun playSound(s: Sound) {
        if (kind.hasAudio) sendV2(true, Dev.USER_IO, 0x07, be16(s.sector) + listOf(0))
    }

    fun stopAudio() {
        if (kind.hasAudio) sendV2(true, Dev.USER_IO, 0x0A)
    }

    fun setVolume(v: Int) {
        if (kind.hasAudio) sendV2(true, Dev.USER_IO, 0x08, listOf(v.coerceIn(0, 255)))
    }

    fun setChatterOn(on: Boolean, now: Long) {
        chatter = on
        if (on) nextChatter = now + Random.nextLong(6_000, 16_000)
    }

    fun setTrophyModeOn(on: Boolean) {
        trophyMode = on
        if (kind.usesV2) sendV2(true, Dev.ANIMATRONICS, 0x2D, listOf(if (on) 1 else 0))
    }

    fun setInteractions(m: Interactions) {
        if (!kind.usesV2) return
        sendV2(true, Dev.ANIMATRONICS, 0x2C, listOf(if (m != Interactions.OFF) 1 else 0)) // animaciones de reposo
        sendV2(true, Dev.ANIMATRONICS, 0x2F, listOf(if (m == Interactions.SOCIAL) 1 else 0)) // droide a droide
    }

    // ------------------------------------------------------------- luces

    fun setLightRgb(ch: LightChannel, r: Int, g: Int, b: Int) {
        when (val s = ch.style) {
            is LightStyle.Rgb -> setV2Leds(s.r or s.g or s.b, listOf(r, g, b))
            LightStyle.V1Main -> sendV1(0x02, 0x20, listOf(r, g, b, 0), quiet = true)
            else -> {}
        }
    }

    fun setLightLevel(ch: LightChannel, level: Int) {
        when (val s = ch.style) {
            is LightStyle.Mono -> setV2Leds(s.mask, listOf(level))
            LightStyle.V1Back -> sendV1(0x02, 0x21, listOf(level))
            else -> {}
        }
    }

    private fun setV2Leds(mask: Int, values: List<Int>) = sendV2(true, Dev.USER_IO, 0x0E, be16(mask) + values, quiet = true)

    fun setAimLight(on: Boolean) {
        when (kind) {
            Kind.BB8 -> sendV1(0x02, 0x21, listOf(if (on) 255 else 0))
            Kind.BB9E -> setV2Leds(0x08, listOf(if (on) 255 else 0))
            else -> {}
        }
    }

    fun resetLights() {
        if (kind.usesV2) sendV2(true, Dev.USER_IO, 0x19)
    }

    // ------------------------------------------------------------- colisiones

    fun configureCollisions(sensitivity: Int = 70) {
        val p = listOf(1, sensitivity, sensitivity, sensitivity, sensitivity, 40)
        if (kind.usesV2) {
            sendV2(true, Dev.SENSORS, 0x11, p)
            sendV2(true, Dev.SENSORS, 0x14, listOf(1))
        } else {
            sendV1(0x02, 0x12, p)
        }
    }

    private fun handleHit() {
        note("¡Colisión!")
        hitPending = true
        if (!reactToHits) return
        kind.sounds.filter { it.group == "Hit" || it.group == "Negative" }.randomOrNull()?.let(::playSound)
    }

    // ------------------------------------------------------------- respuestas

    fun onNotification(data: ByteArray) {
        if (kind.usesV2) decV2.feed(data).forEach(::handleV2) else decV1.feed(data).forEach(::handleV1)
    }

    private fun noteError(e: Int, did: Int, cid: Int) {
        val key = "%02X/%02X e%d".format(did, cid, e)
        // No repetir el mismo error al conducir a 10 Hz.
        if (key == lastError && clock - lastErrorAt < 3000) return
        lastError = key
        lastErrorAt = clock
        note("ERROR $e (${ERROR_NAMES.getOrNull(e) ?: "?"}) en %02X/%02X".format(did, cid))
    }

    private fun handleV2(p: ParsedV2) {
        if (p.isResponse) {
            val cand = probes.remove(p.seq)
            if (cand != null) {
                if (p.error == 0 && !routingConfirmed) {
                    routing = cand
                    routingConfirmed = true
                    routingText = cand.title
                    note("OK direcciones detectadas: ${cand.title}")
                    afterRoutingConfirmed()
                } else {
                    note("probe ${cand.title} -> error ${p.error ?: 0}")
                }
                return
            }
            if (p.error != null && p.error != 0) noteError(p.error, p.did, p.cid)
        }
        when (p.did to p.cid) {
            Dev.POWER to 0x04, Dev.POWER to 0x06 -> p.payload.firstOrNull()?.let {
                battery = mapOf(0 to Battery.CHARGED, 1 to Battery.CHARGING, 2 to Battery.OK, 3 to Battery.OK, 4 to Battery.LOW, 5 to Battery.CRITICAL)[it] ?: Battery.UNKNOWN
            }
            Dev.POWER to 0x03 -> readU16(p.payload, 0)?.let { batteryText = "%.2f V".format(it / 100.0) }
            Dev.SYSTEM_INFO to 0x00 -> versions["Nordic"] = version(p.payload)
            Dev.SYSTEM_INFO to 0x17, Dev.SYSTEM_INFO to 0x18 -> versions["Principal"] = version(p.payload)
            Dev.SENSORS to 0x12 -> handleHit()
            Dev.ANIMATRONICS to 0x11 -> {
                busy = false
                busyUntil = null
            }
            else -> {
                // Los acuses correctos y vacíos (uno por cada orden de conducción) solo meterían ruido.
                if (!(p.isResponse && p.error == 0 && p.payload.isEmpty())) note("<- %02X/%02X %s".format(p.did, p.cid, p.payload.hex()))
            }
        }
    }

    private fun version(b: List<Int>): String {
        val (a, m, r) = Triple(readU16(b, 0), readU16(b, 2), readU16(b, 4))
        return if (a != null && m != null && r != null) "$a.$m.$r" else b.hex()
    }

    private fun setBatteryV1(s: Int) {
        battery = mapOf(1 to Battery.CHARGING, 2 to Battery.OK, 3 to Battery.LOW, 4 to Battery.CRITICAL)[s] ?: Battery.UNKNOWN
    }

    private fun handleV1(p: ParsedV1) {
        if (p.isAsync) {
            when (p.code) {
                0x07 -> handleHit()
                0x01 -> p.payload.firstOrNull()?.let(::setBatteryV1)
                else -> note("<- async %02X %s".format(p.code, p.payload.hex()))
            }
            return
        }
        if (p.code != 0) note("ERROR ${p.code}")
        // Respuesta de GetPowerState: [versión, estado, voltaje(2), cargas(2), tiempo(2)]
        if (p.seq == v1BatterySeq && p.payload.size >= 4) {
            setBatteryV1(p.payload[1])
            readU16(p.payload, 2)?.let { batteryText = "%.2f V".format(it / 100.0) }
        }
    }

    companion object {
        /// Códigos de error de la API v2 (el 8 se observó en un R2-D2 real al conducir durante una animación).
        private val ERROR_NAMES = listOf(
            "correcto", "DID no válido", "CID no válido", "no implementado", "restringido", "longitud de datos incorrecta",
            "falló", "parámetro no válido", "ocupado", "destino no válido", "destino no disponible",
        )
    }
}
