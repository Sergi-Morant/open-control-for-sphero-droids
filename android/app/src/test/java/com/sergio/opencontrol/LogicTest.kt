package com.sergio.opencontrol

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.log10
import kotlin.math.sin
import kotlin.random.Random

/// Mismos vectores que desktop/src/protocol.rs (uno viene de una captura de un R2-D2 real).
class ProtocolTest {
    @Test
    fun v2KnownVectors() {
        val old = Routing(0x11, 0x12, true)
        assertEquals("8D 32 11 01 13 0D 01 9A D8", encodeV2(old, false, Dev.POWER, 0x0D, 1).hex())
        val r = Routing.CANDIDATES[0]
        assertEquals("8D 12 02 16 06 5F 70 D8", encodeV2(r, true, Dev.DRIVING, 0x06, 0x5F).hex())
        assertEquals("8D 12 01 13 04 60 75 D8", encodeV2(r, false, Dev.POWER, 0x04, 0x60).hex())
    }

    @Test
    fun v2EscapingRoundTrips() {
        val enc = encodeV2(Routing.CANDIDATES[0], true, Dev.USER_IO, 0x0E, 9, listOf(0x8D, 0xAB, 0xD8))
        assertTrue(enc.hex().contains("AB 05 AB 23 AB 50"))
        val parsed = DecoderV2().feed(enc)
        assertEquals(1, parsed.size)
        assertEquals(listOf(0x8D, 0xAB, 0xD8), parsed[0].payload)
    }

    @Test
    fun v2DecodesSplitResponse() {
        // Respuesta (flags 0x11: respuesta + target) con error 0, partida en dos notificaciones.
        val body = listOf(0x11, 0x01, 0x10, 0x00, 0x07, 0x00)
        val raw = (listOf(0x8D) + body + listOf(body.sum().inv() and 0xFF, 0xD8)).toBytes()
        val d = DecoderV2()
        assertTrue(d.feed(raw.copyOfRange(0, 3)).isEmpty())
        val p = d.feed(raw.copyOfRange(3, raw.size))
        assertEquals(1, p.size)
        assertTrue(p[0].isResponse)
        assertEquals(listOf(0x10, 0x00, 0x07, 0), listOf(p[0].did, p[0].cid, p[0].seq, p[0].error))
    }

    @Test
    fun v1RollAndDecoders() {
        assertEquals("FF FF 02 30 03 06 64 00 5A 01 00 05", encodeV1(0x02, 0x30, 3, listOf(100, 0x00, 0x5A, 1, 0)).hex())
        val d = DecoderV1()
        val r = d.feed(listOf(0xFF, 0xFF, 0x00, 0x03, 0x01, 0xFB).toBytes())
        assertEquals(listOf(false, 0, 3), listOf(r[0].isAsync, r[0].code, r[0].seq))
        val a = d.feed(listOf(0xFF, 0xFE, 0x07, 0x00, 0x03, 0x01, 0x02, 0x00).toBytes())
        assertEquals(true, a[0].isAsync)
        assertEquals(7, a[0].code)
        assertEquals(listOf(1, 2), a[0].payload)
    }
}

class CatalogTest {
    @Test
    fun namesAreTranslated() {
        assertEquals("Acceso a paneles", Spanish.title("R2_Access Panels"))
        assertEquals("Enfadado", Spanish.title("R2D2_WWM_Angry"))
        assertEquals("Parlanchín 12", Spanish.title("R2_Chatty_12"))
        assertEquals("Cargador 1", Spanish.title("R2D2_Charger1"))
        assertEquals("Chatty", Sound("R2_Chatty_12", 1).group)
        assertEquals(Kind.R2D2, Kind.fromBleName("D2-1A2B"))
        assertNull(Kind.fromBleName("Altavoz"))
        assertTrue(Kind.R2D2.sounds.size > 200 && Kind.BB9E.animations.isNotEmpty())
    }
}

class DroidTest {
    private fun droid(sent: MutableList<ByteArray> = mutableListOf()) =
        Droid("id", "D2-TEST", Kind.R2D2) { sent.add(it) }.also { it.onReady(0) }

    @Test
    fun probesRoutingAndConfirmsOnAnswer() {
        val sent = mutableListOf<ByteArray>()
        val d = droid(sent)
        d.tick(1000)
        // La primera sonda usa el esquema confirmado; se responde con error 0 y el mismo número de secuencia.
        val probe = DecoderV2().feed(sent.last()).single()
        assertEquals(Dev.API_PROCESSOR, probe.did)
        val body = listOf(0x11, 0x01, Dev.API_PROCESSOR, 0x00, probe.seq, 0x00)
        d.onNotification((listOf(0x8D) + body + listOf(body.sum().inv() and 0xFF, 0xD8)).toBytes())
        assertEquals("dest 01/02", d.routingText)
    }

    @Test
    fun driveEncodesSpeedHeadingAndReverse() {
        val sent = mutableListOf<ByteArray>()
        val d = droid(sent)
        d.drive(300, -90, reverse = true)
        val p = DecoderV2().feed(sent.last()).single()
        assertEquals(listOf(Dev.DRIVING, 0x07), listOf(p.did, p.cid))
        assertEquals(listOf(255, 0x01, 0x0E, 1), p.payload) // 270° = 0x010E
    }
}

class FollowTest {
    @Test
    fun gradientPointsWhereSignalGrows() {
        // La señal crece hacia el este (x): rumbo 90°.
        val pts = (0 until 20).map { Follow.Sample((it % 5).toFloat(), (it / 5).toFloat(), -70f + 2f * (it % 5)) }
        val g = Follow.gradient(pts)
        assertNotNull(g)
        assertTrue("rumbo ${g!!.first}", abs(angleDiff(g.first, 90f)) < 5f)
        assertTrue(g.second > 1f)
        // En línea recta alejándose: la pendiente apunta hacia atrás, sin componente lateral inventada.
        val line = (0 until 20).map { Follow.Sample(0f, it.toFloat(), -60f - 0.5f * it) }
        assertTrue(abs(angleDiff(Follow.gradient(line)!!.first, 180f)) < 5f)
        assertNull(Follow.gradient(pts.take(5)))
    }

    /// Mundo simulado: el teléfono está en el origen, la señal cae con la distancia (más ruido) y el droide
    /// se mueve según la última orden. Devuelve si acabó en «he llegado».
    private fun simulate(seed: Int, facing: Float, noise: Float): Boolean {
        val rnd = Random(seed)
        val d = Droid("id", "D2-TEST", Kind.R2D2) {}.also { it.onReady(0) }
        val f = Follow()
        var x = 6f
        var y = 1f
        f.start(d, 0)
        for (step in 0 until 2400) {
            val now = step * 100L
            if (step % 3 == 0) {
                d.rssi = Math.round(-45f - 22f * log10(hypot(x, y).coerceAtLeast(0.2f)) + (rnd.nextFloat() - 0.5f) * 2f * noise)
                d.rssiAt = now
            }
            f.tick(d, now)
            val (speed, heading) = f.command
            val a = Math.toRadians((facing + heading).toDouble())
            val v = speed / 45f * 0.35f // m/s
            x += sin(a).toFloat() * v * 0.1f
            y += cos(a).toFloat() * v * 0.1f
        }
        return f.arrived
    }

    /// Sale a 6 m, mirando hacia cualquier lado, y tiene que acabar junto al teléfono.
    @Test
    fun approachesThePhoneInSimulation() {
        val runs = 60
        val arrived = (0 until runs).count { simulate(it, (it * 47 % 360).toFloat(), 3f) }
        assertTrue("solo llegaron $arrived/$runs", arrived >= runs * 85 / 100)
    }

    @Test
    fun stopsWhenSignalGoesStale() {
        val d = Droid("id", "D2-TEST", Kind.R2D2) {}.also { it.onReady(0) }
        val f = Follow()
        f.start(d, 0)
        for (i in 0 until 10) {
            d.rssi = -70
            d.rssiAt = i * 300L
            f.tick(d, i * 300L)
        }
        assertTrue(f.command.first > 0)
        f.tick(d, 10_000)
        assertEquals(0, f.command.first)
        assertTrue(f.running)
    }
}
