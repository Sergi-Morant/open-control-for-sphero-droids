package com.sergio.opencontrol

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.hypot
import kotlin.math.roundToInt

class Found(val device: BluetoothDevice, val name: String, val kind: Kind, rssi: Int) {
    val address: String get() = device.address
    var rssi by mutableStateOf(rssi)
}

/// Un droide conectado con sus modos automáticos y el estado de sus pantallas.
class Entry(val droid: Droid, val conn: Connection) {
    val emoteLoop = EmoteLoop()
    val patrol = Patrol()
    val draw = DrawDrive()
    val follow = Follow()

    // conducción
    var heading by mutableStateOf(0)
    var maxSpeed by mutableStateOf(0.6f)
    var turbo by mutableStateOf(false)
    var reverse by mutableStateOf(false)
    var stabilize by mutableStateOf(true)
    var dome by mutableStateOf(0f)
    /// Joystick en pantalla: (x, y) con y positiva hacia arriba, o null si no se está tocando.
    var stick: Pair<Float, Float>? = null
    var padAiming by mutableStateOf(false)
    internal var padDriving = false
    internal var padDomeDirty = false
    internal var lastDomeSent = 0L
    /// La pantalla Seguir está a la vista: se pide la señal aunque el modo esté parado.
    var watchSignal = false
    internal var lastRssiRequest = 0L

    /// Detiene todo lo que la app haya puesto en marcha.
    fun stopAll(now: Long) {
        emoteLoop.stop()
        follow.stop(droid)
        patrol.stop(droid)
        draw.abort(droid)
        droid.setChatterOn(false, now)
        droid.stopAnimation()
        droid.stopAudio()
        droid.stop(heading)
    }
}

/// Estado de un mando físico. Lo rellena MainActivity con los eventos del sistema.
class Pad {
    var name by mutableStateOf<String?>(null)
    var left = 0f to 0f
    var rightX = 0f
    var lt = 0f
    var rt = 0f
    val pressed = ArrayDeque<Int>()

    /// Zona muerta radial con reescalado, para que el stick no «tiemble» en reposo.
    private fun deadzone(x: Float, y: Float): Pair<Float, Float> {
        val m = hypot(x, y)
        if (m < 0.15f) return 0f to 0f
        val scaled = ((m - 0.15f) / 0.85f).coerceAtMost(1f)
        return (x / m * scaled) to (y / m * scaled)
    }

    fun onMotion(e: MotionEvent): Boolean {
        if (e.source and InputDevice.SOURCE_JOYSTICK != InputDevice.SOURCE_JOYSTICK || e.action != MotionEvent.ACTION_MOVE) return false
        name = e.device?.name ?: "Mando"
        left = deadzone(e.getAxisValue(MotionEvent.AXIS_X), -e.getAxisValue(MotionEvent.AXIS_Y))
        rightX = deadzone(e.getAxisValue(MotionEvent.AXIS_Z), 0f).first
        lt = maxOf(e.getAxisValue(MotionEvent.AXIS_LTRIGGER), e.getAxisValue(MotionEvent.AXIS_BRAKE))
        rt = maxOf(e.getAxisValue(MotionEvent.AXIS_RTRIGGER), e.getAxisValue(MotionEvent.AXIS_GAS))
        return true
    }

    fun onKey(e: KeyEvent): Boolean {
        if (e.source and InputDevice.SOURCE_GAMEPAD != InputDevice.SOURCE_GAMEPAD || !KeyEvent.isGamepadButton(e.keyCode)) return false
        name = e.device?.name ?: "Mando"
        if (e.action == KeyEvent.ACTION_DOWN && e.repeatCount == 0) pressed.addLast(e.keyCode)
        return true
    }
}

@SuppressLint("MissingPermission") // los permisos se piden antes de buscar o conectar (ver Ui.kt)
class AppState(private val context: Context) {
    val found = mutableStateListOf<Found>()
    val entries = mutableStateListOf<Entry>()
    var activeId by mutableStateOf<String?>(null)
    var scanning by mutableStateOf(false)
    var bluetoothText by mutableStateOf("")
    /// Reloj que avanza diez veces por segundo; las pantallas que dependen del tiempo lo leen.
    var clock by mutableStateOf(now())
    val pad = Pad()

    val active: Entry? get() = entries.firstOrNull { it.droid.id == activeId } ?: entries.firstOrNull()

    private val main = Handler(Looper.getMainLooper())
    private val adapter get() = (context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager)?.adapter
    private var lastTick = now()

    private val ticker = object : Runnable {
        override fun run() {
            tick()
            main.postDelayed(this, 100)
        }
    }

    init {
        main.post(ticker)
    }

    // ------------------------------------------------------------- búsqueda y conexión

    private val scanCallback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val name = result.scanRecord?.deviceName ?: result.device.name ?: return
            val kind = Kind.fromBleName(name) ?: return
            val existing = found.firstOrNull { it.address == result.device.address }
            if (existing != null) existing.rssi = result.rssi else found.add(Found(result.device, name, kind, result.rssi))
        }

        override fun onScanFailed(errorCode: Int) {
            scanning = false
            bluetoothText = "No se pudo buscar (código $errorCode)"
        }
    }

    private val stopScanLater = Runnable { stopScan() }

    fun startScan() {
        val a = adapter
        if (a == null || !a.isEnabled) {
            bluetoothText = "Activa el Bluetooth en los ajustes del teléfono"
            return
        }
        val scanner = a.bluetoothLeScanner ?: return
        found.clear()
        bluetoothText = ""
        scanning = true
        scanner.startScan(null, ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build(), scanCallback)
        main.removeCallbacks(stopScanLater)
        main.postDelayed(stopScanLater, 20_000)
    }

    fun stopScan() {
        main.removeCallbacks(stopScanLater)
        if (scanning) runCatching { adapter?.bluetoothLeScanner?.stopScan(scanCallback) }
        scanning = false
    }

    fun connect(f: Found) {
        stopScan()
        val existing = entries.firstOrNull { it.droid.id == f.address }
        if (existing != null) {
            if (existing.droid.link == Link.DISCONNECTED) existing.conn.connect()
            activeId = f.address
            return
        }
        val conn = Connection(context, f.device, f.kind)
        val droid = Droid(f.address, f.name, f.kind, conn::write)
        conn.droid = droid
        entries.add(Entry(droid, conn))
        activeId = f.address
        conn.connect()
    }

    fun disconnect(e: Entry) {
        e.stopAll(now())
        e.conn.disconnect()
        entries.remove(e)
        if (activeId == e.droid.id) activeId = entries.firstOrNull()?.droid?.id
    }

    // ------------------------------------------------------------- bucle principal

    private fun tick() {
        val now = now()
        val dt = ((now - lastTick) / 1000f).coerceIn(0f, 0.5f)
        lastTick = now
        clock = now
        active?.let { applyPad(it, dt, now) }
        for (e in entries) {
            val d = e.droid
            d.tick(now)
            // Joystick en pantalla: el droide se detiene si deja de recibir órdenes, así que se reenvían a ~10 Hz.
            e.stick?.let { (x, y) ->
                val mag = hypot(x, y).coerceAtMost(1f)
                if (mag > 0.1f) e.heading = headingOf(x, y)
                val cap = if (e.turbo) 1f else e.maxSpeed
                d.drive(if (mag > 0.1f) (mag * 255 * cap).toInt() else 0, e.heading, e.reverse)
            }
            e.emoteLoop.tick(d, now)
            e.patrol.tick(d, now)
            e.draw.tick(d, now)
            e.follow.tick(d, now)
            if (d.isReady && (e.follow.running || e.watchSignal) && now - e.lastRssiRequest >= 350) {
                e.lastRssiRequest = now
                e.conn.readRssi()
            }
        }
    }

    /// Aplica el mando al droide activo. Controles fijos, como los valores por defecto de las otras apps.
    private fun applyPad(e: Entry, dt: Float, now: Long) {
        val d = e.droid
        if (pad.name == null) return
        // Gatillos: RT sube y LT baja la velocidad máxima.
        e.maxSpeed = (e.maxSpeed + (pad.rt - pad.lt) * 0.5f * dt).coerceIn(0.15f, 1f)

        val (lx, ly) = pad.left
        val mag = hypot(lx, ly)
        if (e.padAiming) {
            if (mag > 0f) {
                e.heading = headingOf(lx, ly)
                d.aim(e.heading)
            }
        } else if (mag > 0f) {
            e.heading = headingOf(lx, ly)
            e.padDriving = true
            d.drive((mag * 255 * (if (e.turbo) 1f else e.maxSpeed)).toInt(), e.heading)
        } else if (e.padDriving) {
            e.padDriving = false
            d.stop(e.heading)
        }

        // Stick derecho: la cúpula gira poco a poco y se queda donde se suelta.
        if (d.kind.hasDome) {
            if (pad.rightX != 0f) {
                val target = (e.dome + pad.rightX * abs(pad.rightX) * 120f * dt).coerceIn(-160f, 180f)
                if (target != e.dome) {
                    e.dome = target
                    e.padDomeDirty = true
                }
            }
            if (e.padDomeDirty && now - e.lastDomeSent >= 100) {
                e.lastDomeSent = now
                e.padDomeDirty = false
                d.setDome(e.dome)
            }
        }

        while (true) {
            when (pad.pressed.removeFirstOrNull() ?: break) {
                KeyEvent.KEYCODE_BUTTON_A -> d.legAction(LegAction.THREE_LEGS)
                KeyEvent.KEYCODE_BUTTON_B -> d.legAction(LegAction.TWO_LEGS)
                KeyEvent.KEYCODE_BUTTON_Y -> d.legAction(LegAction.WADDLE)
                KeyEvent.KEYCODE_BUTTON_X -> d.legAction(LegAction.STOP)
                KeyEvent.KEYCODE_BUTTON_START -> e.stopAll(now)
                KeyEvent.KEYCODE_BUTTON_THUMBL -> {
                    // Primera pulsación: modo apuntado. Segunda: la orientación actual se fija como «adelante».
                    if (e.padAiming) {
                        d.setForward()
                        d.setAimLight(false)
                        e.padAiming = false
                        e.heading = 0
                    } else {
                        if (e.padDriving) {
                            e.padDriving = false
                            d.stop(e.heading)
                        }
                        e.padAiming = true
                        d.setAimLight(true)
                    }
                }
                KeyEvent.KEYCODE_BUTTON_THUMBR -> {
                    e.dome = 0f
                    e.padDomeDirty = false
                    d.setDome(0f)
                }
            }
        }
    }

    companion object {
        /// Rumbo (0…359, 0 = adelante, sentido horario) de un stick con y positiva hacia arriba.
        fun headingOf(x: Float, y: Float): Int = Math.toDegrees(atan2(x, y).toDouble()).roundToInt().mod(360)

        /// Permisos que hacen falta para buscar y conectar, según la versión de Android.
        val permissions: Array<String>
            get() = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT)
            } else {
                arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
            }
    }
}
