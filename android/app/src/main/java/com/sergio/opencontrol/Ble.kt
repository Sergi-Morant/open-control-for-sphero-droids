package com.sergio.opencontrol

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothProfile
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import java.util.UUID

private fun uuid(s: String) = UUID.fromString(s)

// API v2 (R2-D2, R2-Q5, BB-9E)
private val V2_API = uuid("00010002-574f-4f20-5370-6865726f2121")
private val V2_ANTIDOS = uuid("00020005-574f-4f20-5370-6865726f2121")
private const val V2_SECRET = "usetheforce...band"
// API v1 (BB-8)
private val V1_ANTIDOS = uuid("22bb746f-2bbd-7554-2d6f-726568705327")
private val V1_TXPOWER = uuid("22bb746f-2bb2-7554-2d6f-726568705327")
private val V1_WAKE = uuid("22bb746f-2bbf-7554-2d6f-726568705327")
private val V1_COMMAND = uuid("22bb746f-2ba1-7554-2d6f-726568705327")
private val V1_RESPONSE = uuid("22bb746f-2ba6-7554-2d6f-726568705327")
private const val V1_SECRET = "011i3"
/// Descriptor estándar para activar las notificaciones de una característica.
private val CCCD = uuid("00002902-0000-1000-8000-00805f9b34fb")

fun now(): Long = SystemClock.elapsedRealtime()

/// Conexión Bluetooth LE con un droide. Android solo admite una operación GATT a la vez, así que todo
/// pasa por una cola. Los avisos del sistema llegan en otro hilo y se reenvían al principal.
@SuppressLint("MissingPermission") // los permisos se piden antes de buscar o conectar (ver Ui.kt)
@Suppress("DEPRECATION") // las variantes antiguas de escritura funcionan en todas las versiones que admitimos
class Connection(private val context: Context, private val device: BluetoothDevice, private val kind: Kind) {
    lateinit var droid: Droid

    private sealed interface Op {
        class Write(val ch: BluetoothGattCharacteristic, val data: ByteArray, val type: Int) : Op
        class Notify(val ch: BluetoothGattCharacteristic) : Op
        data object Ready : Op
    }

    private val main = Handler(Looper.getMainLooper())
    private var gatt: BluetoothGatt? = null
    private var command: BluetoothGattCharacteristic? = null
    private var commandType = BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
    private val queue = ArrayDeque<Op>()
    private var inFlight = false
    private var servicesRequested = false
    private val timeout = Runnable { opDone() }

    fun connect() {
        droid.link = Link.CONNECTING
        droid.status = "Conectando…"
        gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE)
    }

    fun disconnect() {
        gatt?.disconnect()
        // Por si el sistema no llega a avisar de la desconexión.
        main.postDelayed({ close() }, 1500)
    }

    fun write(data: ByteArray) {
        val ch = command ?: return
        // Si el enlace va lento, no se acumulan órdenes viejas de conducción.
        if (queue.size > 60) queue.removeFirst()
        enqueue(Op.Write(ch, data, commandType))
    }

    fun readRssi() {
        gatt?.readRemoteRssi()
    }

    private fun close() {
        main.removeCallbacks(timeout)
        queue.clear()
        inFlight = false
        command = null
        val g = gatt ?: return
        gatt = null
        g.close()
        if (droid.link != Link.DISCONNECTED) droid.onDisconnected()
    }

    private fun enqueue(op: Op) {
        queue.addLast(op)
        pump()
    }

    private fun pump() {
        val g = gatt ?: return
        while (!inFlight) {
            val op = queue.removeFirstOrNull() ?: return
            val started = when (op) {
                is Op.Write -> {
                    op.ch.writeType = op.type
                    op.ch.value = op.data
                    g.writeCharacteristic(op.ch)
                }
                is Op.Notify -> {
                    g.setCharacteristicNotification(op.ch, true)
                    val d = op.ch.getDescriptor(CCCD)
                    if (d == null) false else {
                        d.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                        g.writeDescriptor(d)
                    }
                }
                Op.Ready -> {
                    droid.onReady(now())
                    false
                }
            }
            if (started) {
                inFlight = true
                // Si el sistema no confirma la operación, no se bloquea la cola para siempre.
                main.postDelayed(timeout, 2000)
            }
        }
    }

    private fun opDone() {
        main.removeCallbacks(timeout)
        inFlight = false
        pump()
    }

    private fun setup(g: BluetoothGatt) {
        val chars = HashMap<UUID, BluetoothGattCharacteristic>()
        for (s in g.services) {
            droid.note("servicio ${s.uuid}")
            for (c in s.characteristics) chars[c.uuid] = c
        }
        droid.status = "Preparando conexión…"
        val missing = { droid.status = "No es un droide compatible (faltan características)"; droid.note(droid.status) }
        if (kind.usesV2) {
            val api = chars[V2_API]
            val anti = chars[V2_ANTIDOS]
            if (api == null || anti == null) return missing()
            useCommand(api)
            enqueue(Op.Notify(api))
            enqueue(Op.Write(anti, V2_SECRET.toByteArray(), BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT))
        } else {
            val resp = chars[V1_RESPONSE]
            val anti = chars[V1_ANTIDOS]
            val cmd = chars[V1_COMMAND]
            if (resp == null || anti == null || cmd == null) return missing()
            useCommand(cmd)
            enqueue(Op.Notify(resp))
            // BB-8: anti-DoS → potencia de transmisión → despertar CPU
            enqueue(Op.Write(anti, V1_SECRET.toByteArray(), BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT))
            chars[V1_TXPOWER]?.let { enqueue(Op.Write(it, byteArrayOf(0x07), BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT)) }
            chars[V1_WAKE]?.let { enqueue(Op.Write(it, byteArrayOf(0x01), BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT)) }
        }
        enqueue(Op.Ready)
    }

    private fun useCommand(ch: BluetoothGattCharacteristic) {
        command = ch
        commandType = if (ch.properties and BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE != 0) {
            BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
        } else {
            BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
        }
    }

    private fun discover(g: BluetoothGatt) {
        if (servicesRequested) return
        servicesRequested = true
        droid.status = "Descubriendo servicios…"
        g.discoverServices()
    }

    private val callback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            main.post {
                if (newState == BluetoothProfile.STATE_CONNECTED && status == BluetoothGatt.GATT_SUCCESS) {
                    servicesRequested = false
                    // Paquetes más grandes si el droide lo admite; si no responde, se sigue igualmente.
                    if (!g.requestMtu(185)) discover(g) else main.postDelayed({ if (gatt === g) discover(g) }, 1500)
                } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                    val failed = droid.link == Link.CONNECTING
                    close()
                    if (failed) {
                        droid.status = "No se pudo conectar (código $status)"
                        droid.note(droid.status)
                    }
                }
            }
        }

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            main.post { discover(g) }
        }

        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
            main.post {
                if (status == BluetoothGatt.GATT_SUCCESS) setup(g) else droid.status = "Error al descubrir servicios (código $status)"
            }
        }

        override fun onCharacteristicWrite(g: BluetoothGatt, c: BluetoothGattCharacteristic, status: Int) {
            main.post {
                if (status != BluetoothGatt.GATT_SUCCESS && !droid.isReady) {
                    droid.status = "Error de escritura (código $status)"
                    droid.note(droid.status)
                }
                opDone()
            }
        }

        override fun onDescriptorWrite(g: BluetoothGatt, d: BluetoothGattDescriptor, status: Int) {
            main.post { opDone() }
        }

        @Deprecated("En Android 13 o superior el sistema llama a esta misma función desde la nueva")
        override fun onCharacteristicChanged(g: BluetoothGatt, c: BluetoothGattCharacteristic) {
            val data = c.value?.copyOf() ?: return
            main.post { droid.onNotification(data) }
        }

        override fun onReadRemoteRssi(g: BluetoothGatt, rssi: Int, status: Int) {
            if (status != BluetoothGatt.GATT_SUCCESS || rssi == 127) return // 127 = no disponible
            main.post {
                droid.rssi = rssi
                droid.rssiAt = now()
            }
        }
    }
}
