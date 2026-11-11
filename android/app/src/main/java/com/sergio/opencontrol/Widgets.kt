package com.sergio.opencontrol

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.ceil
import kotlin.math.hypot

/// Aspecto de la app: los mismos colores y piezas que en escritorio (desktop/src/theme.rs) y iPhone.
object Th {
    val bg = Color(0xFF0F1218)
    val panel = Color(0xFF141820)
    val card = Color(0xFF1C212C)
    val border = Color(0xFF2C3444)
    val widget = Color(0xFF2A3242)
    val well = Color(0xFF0A0C10)
    val text = Color(0xFFE4E8F0)
    val muted = Color(0xFF8C96AA)
    val accent = Color(0xFF408CF0)
    val accentSoft = Color(0xFF244476)
    val green = Color(0xFF4CC38A)
    val amber = Color(0xFFEEB048)
    val red = Color(0xFFD64842)

    /// Color orientativo para una intensidad de señal en dBm.
    fun signal(dbm: Float): Color = if (dbm >= -60f) green else if (dbm >= -75f) amber else red
}

/// Pantalla desplazable; dentro van las tarjetas.
@Composable
fun Screen(content: @Composable ColumnScope.() -> Unit) {
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        content = content,
    )
}

/// Tarjeta con título opcional que ocupa todo el ancho.
@Composable
fun Panel(title: String? = null, content: @Composable ColumnScope.() -> Unit) {
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(Th.card)
            .border(1.dp, Th.border, RoundedCornerShape(14.dp)).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (title != null) Text(title, fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
        content()
    }
}

/// Texto secundario de ayuda.
@Composable
fun Hint(text: String, color: Color = Th.muted) {
    Text(text, color = color, fontSize = 13.sp, lineHeight = 18.sp)
}

/// Botón grande de arrancar/parar un modo.
@Composable
fun StartStop(on: Boolean, start: String, stop: String, modifier: Modifier = Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        modifier.fillMaxWidth().height(50.dp).alpha(if (enabled) 1f else 0.45f).clip(RoundedCornerShape(12.dp))
            .background(if (on) Th.red else Th.accent).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(if (on) stop else start, color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

/// Botón secundario relleno (acciones sueltas, emotes, sonidos).
@Composable
fun Chip(text: String, modifier: Modifier = Modifier, fill: Color = Th.widget, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        modifier.heightIn(min = 40.dp).alpha(if (enabled) 1f else 0.45f).clip(RoundedCornerShape(10.dp)).background(fill)
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 12.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = if (fill == Th.widget) Th.text else Color.White, fontSize = 14.sp)
    }
}

/// Etiqueta redondeada con un punto de color.
@Composable
fun Pill(color: Color, text: String) {
    Row(
        Modifier.clip(CircleShape).background(Th.panel).border(1.dp, Th.border, CircleShape).padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
        Text(text, fontSize = 13.sp)
    }
}

/// Cuatro barras de cobertura.
@Composable
fun SignalBars(dbm: Float?) {
    val lit = dbm?.let { ceil(((it + 92f) / 9f).coerceIn(0f, 4f)).toInt() } ?: 0
    val color = dbm?.let(Th::signal) ?: Th.border
    Canvas(Modifier.size(width = 26.dp, height = 18.dp)) {
        for (i in 0 until 4) {
            val h = (5 + i * 4).dp.toPx()
            drawRoundRect(
                if (i < lit) color else Th.border,
                topLeft = Offset((i * 7).dp.toPx(), size.height - h),
                size = Size(5.dp.toPx(), h),
                cornerRadius = CornerRadius(1.5.dp.toPx()),
            )
        }
    }
}

/// Gráfica sencilla de una serie en dBm con líneas de referencia (valor, color, rótulo).
@Composable
fun SignalChart(values: List<Float>, capacity: Int, marks: List<Triple<Float, Color, String>>) {
    Canvas(
        Modifier.fillMaxWidth().height(140.dp).clip(RoundedCornerShape(10.dp)).background(Th.well).border(1.dp, Th.border, RoundedCornerShape(10.dp)),
    ) {
        val pad = 8.dp.toPx()
        val (lo, hi) = -95f to -30f
        fun y(v: Float) = size.height - pad - (v.coerceIn(lo, hi) - lo) / (hi - lo) * (size.height - 2 * pad)
        val paint = android.graphics.Paint().apply {
            isAntiAlias = true
            textSize = 11.sp.toPx()
            textAlign = android.graphics.Paint.Align.RIGHT
        }
        for ((v, color, label) in marks) {
            drawLine(color.copy(alpha = 0.7f), Offset(pad, y(v)), Offset(size.width - pad, y(v)), 1.dp.toPx())
            paint.color = color.toArgb()
            drawContext.canvas.nativeCanvas.drawText(label, size.width - pad - 2.dp.toPx(), y(v) - 3.dp.toPx(), paint)
        }
        if (values.size < 2) {
            paint.color = Th.muted.toArgb()
            paint.textAlign = android.graphics.Paint.Align.CENTER
            paint.textSize = 13.sp.toPx()
            drawContext.canvas.nativeCanvas.drawText("Sin medidas todavía", size.width / 2, size.height / 2, paint)
            return@Canvas
        }
        val step = (size.width - 2 * pad) / (maxOf(capacity, 2) - 1)
        val x0 = size.width - pad - step * (values.size - 1)
        val path = Path()
        values.forEachIndexed { i, v -> if (i == 0) path.moveTo(x0, y(v)) else path.lineTo(x0 + step * i, y(v)) }
        drawPath(path, Th.accent, style = Stroke(2.dp.toPx()))
        drawCircle(Th.accent, 4.dp.toPx(), Offset(size.width - pad, y(values.last())))
    }
}

/// Selector de subpantalla en la parte superior de una pestaña.
@Composable
fun SegmentBar(options: List<String>, selected: Int, onSelect: (Int) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 8.dp).clip(RoundedCornerShape(10.dp)).background(Th.widget).padding(3.dp),
    ) {
        options.forEachIndexed { i, name ->
            Box(
                Modifier.weight(1f).height(36.dp).clip(RoundedCornerShape(8.dp)).background(if (i == selected) Th.accent else Color.Transparent)
                    .clickable { onSelect(i) },
                contentAlignment = Alignment.Center,
            ) {
                Text(name, fontSize = 14.sp, fontWeight = if (i == selected) FontWeight.SemiBold else FontWeight.Normal, color = if (i == selected) Color.White else Th.text)
            }
        }
    }
}

@Composable
fun LabeledSlider(label: String, value: Float, range: ClosedFloatingPointRange<Float>, onDone: () -> Unit = {}, onChange: (Float) -> Unit) {
    Column {
        Text(label, fontSize = 14.sp)
        Slider(value = value, onValueChange = onChange, valueRange = range, onValueChangeFinished = onDone)
    }
}

@Composable
fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, Modifier.weight(1f), fontSize = 15.sp)
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

/// Fila «etiqueta: valor» para fichas de información.
@Composable
fun InfoRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth()) {
        Text(label, Modifier.width(150.dp), color = Th.muted, fontSize = 14.sp)
        Text(value, fontSize = 14.sp)
    }
}

/// Joystick en pantalla con marca del rumbo. `onMove` recibe (x, y) en −1…1 con y positiva hacia arriba.
@Composable
fun Joystick(heading: Int, onMove: (Float, Float) -> Unit, onEnd: () -> Unit) {
    var knob by remember { mutableStateOf(Offset.Zero) }
    val move by rememberUpdatedState(onMove)
    val end by rememberUpdatedState(onEnd)
    Canvas(
        Modifier.size(230.dp).pointerInput(Unit) {
            val radius = size.width / 2f - 10.dp.toPx()
            val center = Offset(size.width / 2f, size.height / 2f)
            fun update(pos: Offset) {
                var d = pos - center
                val m = hypot(d.x, d.y)
                if (m > radius) d *= radius / m
                knob = d
                move(d.x / radius, -d.y / radius)
            }
            awaitEachGesture {
                val down = awaitFirstDown()
                update(down.position)
                while (true) {
                    val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                    if (!change.pressed) break
                    update(change.position)
                    change.consume()
                }
                knob = Offset.Zero
                end()
            }
        },
    ) {
        val r = size.width / 2f - 10.dp.toPx()
        drawCircle(Th.well, r)
        drawCircle(Th.border, r, style = Stroke(2.dp.toPx()))
        drawCircle(Th.border, r / 2, style = Stroke(1.dp.toPx()))
        drawLine(Th.border, Offset(center.x - r, center.y), Offset(center.x + r, center.y), 1.dp.toPx())
        drawLine(Th.border, Offset(center.x, center.y - r), Offset(center.x, center.y + r), 1.dp.toPx())
        // Marca del rumbo en el borde.
        rotate(heading.toFloat()) {
            drawLine(Th.amber, Offset(center.x, center.y - r), Offset(center.x, center.y - r + 14.dp.toPx()), 4.dp.toPx())
        }
        drawCircle(if (knob == Offset.Zero) Th.accent.copy(alpha = 0.85f) else Th.accent, 34.dp.toPx(), center + knob)
        drawCircle(Color.White.copy(alpha = 0.15f), 34.dp.toPx(), center + knob, style = Stroke(2.dp.toPx()))
    }
}
