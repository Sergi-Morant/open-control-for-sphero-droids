package com.sergio.opencontrol

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.random.Random

// Modos automáticos. Son máquinas de estados que se avanzan con `tick` unas diez veces por segundo,
// igual que en la app de escritorio (desktop/src/controllers.rs). `now` son milisegundos monótonos.

/// Diferencia b − a en (−180, 180].
fun angleDiff(a: Float, b: Float): Float {
    var d = (b - a) % 360f
    if (d > 180f) d -= 360f else if (d < -180f) d += 360f
    return d
}

// ---------------------------------------------------------------- emotes automáticos

enum class LoopMode { WWM, EMOTIONS }

class EmoteLoop {
    var running by mutableStateOf<LoopMode?>(null)
    var wwmInterval by mutableStateOf(15f)
    var emotionsInterval by mutableStateOf(15f)
    var includeIonBlast by mutableStateOf(false)
    var lastPlayed by mutableStateOf("")
    private var nextAt = 0L
    private var last: Int? = null

    fun pool(kind: Kind, mode: LoopMode): List<Animation> = kind.animations.filter {
        when (mode) {
            LoopMode.WWM -> it.category == "WWM"
            LoopMode.EMOTIONS -> it.category == "Emote" && (includeIonBlast || !it.isIonBlast)
        }
    }

    fun start(mode: LoopMode, now: Long) {
        running = mode
        nextAt = now
    }

    /// Para el bucle. La animación en curso termina sola.
    fun stop() {
        running = null
    }

    fun tick(d: Droid, now: Long) {
        val mode = running ?: return
        if (!d.isReady) {
            running = null
            return
        }
        // Si la animación anterior dura más que el intervalo, se espera a que acabe.
        if (now < nextAt || d.busy) return
        val all = pool(d.kind, mode)
        val a = (if (all.size > 1) all.filter { it.index != last } else all).randomOrNull() ?: return
        d.playAnimation(a, now)
        last = a.index
        lastPlayed = Spanish.title(a.name)
        nextAt = now + ((if (mode == LoopMode.WWM) wwmInterval else emotionsInterval) * 1000).toLong()
    }
}

// ---------------------------------------------------------------- patrulla

class Patrol {
    private sealed interface Phase {
        data object Forward : Phase
        data class Backing(val until: Long) : Phase
        data class Settling(val until: Long) : Phase
        data class Alarm(val until: Long) : Phase
    }

    var running by mutableStateOf(false)
    var speed by mutableStateOf(70f)
    val entries = mutableStateListOf<String>()
    private var phase: Phase = Phase.Forward
    private var heading = 0
    private var nextAlarm = 0L
    private var started = 0L
    private var clock = 0L

    private fun log(s: String) {
        entries.add(0, "[%6.1f s] %s".format((clock - started) / 1000.0, s))
        if (entries.size > 200) entries.removeRange(200, entries.size)
    }

    fun start(d: Droid, now: Long) {
        running = true
        started = now
        clock = now
        heading = 0
        phase = Phase.Forward
        nextAlarm = now + Random.nextLong(20_000, 35_000)
        d.configureCollisions()
        d.takeHit()
        log("Patrulla iniciada con ${d.name}")
    }

    fun stop(d: Droid) {
        if (running) {
            d.stop(heading)
            log("Patrulla detenida")
        }
        running = false
    }

    fun tick(d: Droid, now: Long) {
        if (!running) return
        clock = now
        if (!d.isReady) {
            running = false
            return
        }
        if (d.takeHit() && phase == Phase.Forward) {
            log("Colisión: retrocediendo")
            phase = Phase.Backing(now + 800)
        }
        when (val p = phase) {
            Phase.Forward -> {
                if (now >= nextAlarm) {
                    d.stop(heading)
                    val pool = d.kind.animations.filter { it.category == "Patrol" }.ifEmpty { d.kind.animations.filter { it.category == "Emote" } }
                    val a = pool.randomOrNull()
                    if (a == null) {
                        phase = Phase.Settling(now + 500)
                    } else {
                        d.playAnimation(a, now)
                        log("Alarma: ${Spanish.title(a.name)}")
                        phase = Phase.Alarm(now + ((a.seconds + 0.5f) * 1000).toLong())
                    }
                    nextAlarm = now + Random.nextLong(20_000, 40_000)
                } else {
                    d.drive(speed.toInt(), heading)
                }
            }
            is Phase.Backing -> {
                d.drive((speed * 0.6f).toInt(), heading, reverse = true)
                if (now >= p.until) {
                    val turn = Random.nextInt(100, 201) * if (Random.nextBoolean()) 1 else -1
                    heading = (heading + turn).mod(360)
                    log("Giro de $turn°")
                    phase = Phase.Settling(now + 800)
                }
            }
            is Phase.Settling -> {
                d.drive(0, heading)
                if (now >= p.until) phase = Phase.Forward
            }
            is Phase.Alarm -> if (now >= p.until) phase = Phase.Forward
        }
    }
}

// ---------------------------------------------------------------- dibujar y conducir

/// Dibujas un recorrido y el droide lo sigue. Sin sensores de posición se calcula por tiempo:
/// la distancia recorrida depende de la calibración de velocidad.
class DrawDrive {
    private data class Segment(val heading: Int, val seconds: Float, val speed: Int)

    val points = mutableStateListOf<Pair<Float, Float>>()
    var running by mutableStateOf(false)
    var progress by mutableStateOf(0f)
    var speed by mutableStateOf(80f)
    var cmPerSecond by mutableStateOf(30f)
    var cmPerPoint by mutableStateOf(0.4f)
    private var segments = listOf<Segment>()
    private var started = 0L
    private var total = 0f

    fun clear(d: Droid) {
        abort(d)
        points.clear()
    }

    fun run(d: Droid, now: Long) {
        abort(d)
        segments = buildSegments()
        total = segments.sumOf { it.seconds.toDouble() }.toFloat()
        if (total <= 0f) return
        d.configureCollisions()
        d.takeHit()
        started = now
        running = true
    }

    fun abort(d: Droid) {
        if (running) d.stop()
        running = false
        progress = 0f
    }

    fun tick(d: Droid, now: Long) {
        if (!running) return
        if (!d.isReady || d.takeHit()) {
            abort(d)
            return
        }
        val t = (now - started) / 1000f
        progress = (t / total).coerceAtMost(1f)
        var acc = 0f
        for (s in segments) {
            if (t < acc + s.seconds) {
                d.drive(s.speed, s.heading)
                return
            }
            acc += s.seconds
        }
        abort(d)
    }

    /// Simplifica el trazo y lo convierte en tramos (rumbo, duración).
    private fun buildSegments(): List<Segment> {
        val pts = mutableListOf<Pair<Float, Float>>()
        for (p in points) {
            val l = pts.lastOrNull()
            if (l == null || hypot(l.first - p.first, l.second - p.second) >= 14f) pts.add(p)
        }
        if (pts.size < 2) return emptyList()
        val merged = mutableListOf<Pair<Float, Float>>() // (rumbo, distancia en puntos)
        for ((a, b) in pts.zipWithNext()) {
            val (dx, dy) = (b.first - a.first) to (b.second - a.second)
            var deg = Math.toDegrees(atan2(dx, -dy).toDouble()).toFloat()
            if (deg < 0f) deg += 360f
            val dist = hypot(dx, dy)
            val last = merged.lastOrNull()
            if (last != null && abs(angleDiff(last.first, deg)) < 15f) merged[merged.size - 1] = last.first to last.second + dist
            else merged.add(deg to dist)
        }
        val out = mutableListOf<Segment>()
        var prev = merged[0].first
        for ((deg, dist) in merged) {
            if (abs(angleDiff(prev, deg)) > 30f) out.add(Segment(deg.toInt(), 0.4f, 0))
            out.add(Segment(deg.toInt(), dist * cmPerPoint / cmPerSecond, speed.toInt()))
            prev = deg
        }
        return out
    }
}

// ---------------------------------------------------------------- seguir al móvil

/// Sigue al teléfono usando solo la intensidad de la señal Bluetooth (RSSI).
///
/// El RSSI no da dirección y fluctúa mucho. El droide avanza zigzagueando y apunta cada medida junto a su
/// posición estimada (se calcula con las órdenes enviadas, sin sensores). Con las medidas de los últimos
/// segundos se ajusta un plano: su pendiente indica hacia dónde crece la señal, y hacia ahí se corrige el
/// rumbo. Frena al acercarse, se para si deja de recibir medidas y sigue siendo aproximado.
/// Mismo algoritmo que `Follow` en desktop/src/controllers.rs.
class Follow {
    private sealed interface Phase {
        data object Forward : Phase
        data class Settling(val until: Long) : Phase
        data object Arrived : Phase
    }

    data class Sample(val x: Float, val y: Float, val rssi: Float)

    var running by mutableStateOf(false)
    /// Señal filtrada (dBm). Se actualiza siempre que haya medidas, también con el modo parado.
    var smoothed by mutableStateOf<Float?>(null)
    var status by mutableStateOf("Detenido")
    /// dBm; por encima de esto se considera «ya he llegado».
    var nearThreshold by mutableStateOf(-55f)
    var speed by mutableStateOf(45f)
    /// Últimas señales filtradas, para la gráfica.
    val history = mutableStateListOf<Float>()
    /// Última orden enviada: (velocidad, rumbo).
    var command = 0 to 0
        private set
    val arrived: Boolean get() = phase == Phase.Arrived

    private var phase: Phase = Phase.Forward
    private var target = 0f
    private var weaveSign = 1f
    private var heading = 0
    private var pos = 0f to 0f
    private var lastTick = 0L
    private val samples = ArrayDeque<Sample>()
    private val recent = ArrayDeque<Float>()
    private var seenAt: Long? = null
    private var legEnd = 0L
    private var retreated = false
    private var stalled = false

    fun start(d: Droid, now: Long) {
        running = true
        phase = Phase.Forward
        target = 0f
        weaveSign = 1f
        pos = 0f to 0f
        samples.clear()
        retreated = false
        stalled = false
        lastTick = now
        newLeg(now)
        status = "Esperando señal…"
        d.configureCollisions()
        d.takeHit()
    }

    fun stop(d: Droid) {
        if (running) halt(d)
        running = false
        status = "Detenido"
    }

    /// Usa la señal actual como «cerca».
    fun calibrateNear() {
        smoothed?.let { nearThreshold = Math.round(it - 3f).toFloat().coerceIn(-80f, -35f) }
    }

    /// Hay medidas recientes de señal.
    fun signalFresh(d: Droid, now: Long): Boolean = d.rssiAt?.let { now - it < STALE } ?: false

    private fun newLeg(now: Long) {
        legEnd = now + LEG
        weaveSign = -weaveSign
        heading = (target + WEAVE * weaveSign).mod(360f).toInt()
    }

    private fun go(d: Droid, speed: Int) {
        command = speed to heading
        d.drive(speed, heading)
    }

    private fun halt(d: Droid) {
        command = 0 to heading
        d.stop(heading)
    }

    /// Cambio brusco de rumbo: gira sobre sí mismo un momento antes de seguir.
    private fun turnTo(newTarget: Float, now: Long, ms: Long) {
        target = newTarget.mod(360f)
        heading = target.toInt()
        phase = Phase.Settling(now + ms)
    }

    /// Incorpora la medida nueva, si la hay: mediana de 3 (quita picos) y media exponencial.
    private fun observe(d: Droid) {
        val r = d.rssi ?: return
        val at = d.rssiAt ?: return
        if (seenAt == at) return
        seenAt = at
        recent.addLast(r.toFloat())
        if (recent.size > 3) recent.removeFirst()
        val median = recent.sorted()[recent.size / 2]
        val s = smoothed?.let { 0.65f * it + 0.35f * median } ?: median
        smoothed = s
        history.add(s)
        if (history.size > HISTORY) history.removeAt(0)
        if (running && phase == Phase.Forward) {
            // Para el ajuste se usa la medida sin suavizar: suavizarla la retrasaría respecto a la posición.
            samples.addLast(Sample(pos.first, pos.second, r.toFloat()))
            if (samples.size > FIT_WINDOW) samples.removeFirst()
        }
    }

    fun tick(d: Droid, now: Long) {
        val dt = ((now - lastTick) / 1000f).coerceIn(0f, 0.5f)
        lastTick = now
        if (!d.isReady) {
            running = false
            smoothed = null
            recent.clear()
            return
        }
        // Posición estimada: lo que se le ha mandado hacer desde el tick anterior.
        if (running && command.first > 0) {
            val a = Math.toRadians(command.second.toDouble())
            val step = command.first * dt
            pos = (pos.first + sin(a).toFloat() * step) to (pos.second + cos(a).toFloat() * step)
        }
        observe(d)
        if (!running) return
        // Sin medidas no se puede decidir nada: mejor quieto que avanzando a ciegas.
        if (!signalFresh(d, now)) {
            if (!stalled) {
                stalled = true
                halt(d)
            }
            status = if (d.rssiAt == null) "Esperando señal…" else "Sin lectura de señal: esperando…"
            return
        }
        if (stalled) {
            stalled = false
            newLeg(now)
        }
        if (d.takeHit()) {
            samples.clear()
            turnTo(heading + Random.nextInt(120, 241).toFloat(), now, 1000)
            status = "Obstáculo: cambio de rumbo"
        }
        val s = smoothed ?: return
        when (val p = phase) {
            Phase.Arrived -> {
                status = "He llegado (${s.toInt()} dBm)"
                if (s < nearThreshold - HYSTERESIS) {
                    phase = Phase.Forward
                    samples.clear()
                    newLeg(now)
                }
            }
            is Phase.Settling -> {
                go(d, 0) // gira sobre sí mismo hacia el rumbo nuevo
                if (now >= p.until) {
                    phase = Phase.Forward
                    newLeg(now)
                }
            }
            Phase.Forward -> {
                if (s >= nearThreshold) {
                    phase = Phase.Arrived
                    halt(d)
                    status = "He llegado (${s.toInt()} dBm)"
                    return
                }
                if (s <= LOST_LEVEL) {
                    if (retreated) {
                        stop(d)
                        status = "Señal muy débil (${s.toInt()} dBm): parado para no perderlo"
                    } else {
                        // Una oportunidad: deshacer el camino antes de rendirse.
                        retreated = true
                        samples.clear()
                        turnTo(target + 180f, now, 700)
                        status = "Señal muy débil: vuelvo sobre mis pasos"
                    }
                    return
                }
                if (s > LOST_LEVEL + 6f) retreated = false
                // Frena al acercarse para no pasarse de largo.
                val slow = ((nearThreshold - s) / 10f).coerceIn(0.5f, 1f)
                go(d, (speed * slow).toInt())
                if (now < legEnd) return
                status = "Acercándome… (${s.toInt()} dBm)"
                val g = gradient(samples)
                if (g != null && g.second >= FIT_MIN_STRENGTH) {
                    val diff = angleDiff(target, g.first)
                    if (abs(diff) > 100f) {
                        turnTo(g.first, now, 700)
                        status = "La señal cae: media vuelta"
                        return
                    }
                    target = (target + 0.7f * diff).mod(360f)
                    if (abs(diff) > 20f) status = "Corrigiendo rumbo (${s.toInt()} dBm)"
                }
                newLeg(now)
            }
        }
    }

    companion object {
        /// Duración de cada tramo del zigzag (ms).
        private const val LEG = 1800L
        /// Grados que se desvía cada tramo a un lado y a otro del rumbo.
        private const val WEAVE = 30f
        /// Medidas que se usan para estimar la pendiente (unos 12 s).
        private const val FIT_WINDOW = 36
        private const val FIT_MIN = 10
        /// La pendiente tiene que explicar al menos estos dB a lo largo del recorrido para fiarse de ella.
        private const val FIT_MIN_STRENGTH = 0.8f
        const val LOST_LEVEL = -85f
        /// Sin medidas durante este tiempo (ms), el droide se para.
        private const val STALE = 2500L
        /// dB que tiene que alejarse la señal del umbral para volver a moverse tras llegar.
        private const val HYSTERESIS = 5f
        const val HISTORY = 160

        /// Ajusta un plano a las medidas y devuelve (rumbo hacia el que crece la señal, dB que explica).
        fun gradient(samples: Collection<Sample>): Pair<Float, Float>? {
            val n = samples.size
            if (n < FIT_MIN) return null
            val mx = samples.sumOf { it.x.toDouble() } / n
            val my = samples.sumOf { it.y.toDouble() } / n
            val mr = samples.sumOf { it.rssi.toDouble() } / n
            var sxx = 0.0
            var sxy = 0.0
            var syy = 0.0
            var sxr = 0.0
            var syr = 0.0
            for (s in samples) {
                val (dx, dy, dr) = Triple(s.x - mx, s.y - my, s.rssi - mr)
                sxx += dx * dx
                sxy += dx * dy
                syy += dy * dy
                sxr += dx * dr
                syr += dy * dr
            }
            val spread = sxx + syy
            if (spread < 1e-3) return null
            // Regularización: si el recorrido es casi una línea, la pendiente lateral se queda en cero en vez de dispararse.
            val ridge = 0.08 * spread
            val (a, d) = (sxx + ridge) to (syy + ridge)
            val det = a * d - sxy * sxy
            val gx = (d * sxr - sxy * syr) / det
            val gy = (a * syr - sxy * sxr) / det
            val bearing = Math.toDegrees(atan2(gx, gy)).toFloat().mod(360f)
            return bearing to (hypot(gx, gy) * sqrt(spread / n)).toFloat()
        }
    }
}
