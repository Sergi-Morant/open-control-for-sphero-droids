package com.sergio.opencontrol

import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun AppUi(app: AppState) {
    val colors = darkColorScheme(
        primary = Th.accent, onPrimary = Color.White, background = Th.bg, onBackground = Th.text,
        surface = Th.card, onSurface = Th.text, surfaceVariant = Th.widget, onSurfaceVariant = Th.muted,
        secondaryContainer = Th.accentSoft, onSecondaryContainer = Th.text, error = Th.red, outline = Th.border,
    )
    MaterialTheme(colorScheme = colors) {
        Surface(Modifier.fillMaxSize(), color = Th.bg) {
            Box(Modifier.safeDrawingPadding()) {
                var showConnect by remember { mutableStateOf(false) }
                val e = app.active
                if (e == null || showConnect) {
                    ConnectScreen(app, onClose = if (e != null) ({ showConnect = false }) else null)
                } else {
                    MainScreen(app, e, onAdd = { showConnect = true })
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------- conexión

@Composable
private fun ConnectScreen(app: AppState, onClose: (() -> Unit)?) {
    val context = LocalContext.current
    var denied by remember { mutableStateOf(false) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
        denied = !result.values.all { it }
        if (!denied) app.startScan()
    }
    val scan = {
        val missing = AppState.permissions.filter { context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }
        if (missing.isEmpty()) app.startScan() else launcher.launch(missing.toTypedArray())
    }
    Screen {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Droides", Modifier.weight(1f), fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
            if (onClose != null) Chip("Cerrar", onClick = onClose)
        }
        Panel {
            Box(
                Modifier.fillMaxWidth().height(50.dp).clip(RoundedCornerShape(12.dp)).background(Th.accent.copy(alpha = if (app.scanning) 0.5f else 1f))
                    .clickable(enabled = !app.scanning, onClick = scan),
                contentAlignment = Alignment.Center,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (app.scanning) CircularProgressIndicator(Modifier.size(18.dp), color = Color.White, strokeWidth = 2.dp)
                    Text(if (app.scanning) "Buscando…" else "Buscar droides", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                }
            }
            if (denied) Hint("Sin el permiso de Bluetooth (en Android 11 o anterior, el de ubicación) la app no puede buscar droides. Concédelo en los ajustes de la app.", Th.amber)
            if (app.bluetoothText.isNotEmpty()) Hint(app.bluetoothText, Th.amber)
            Hint("Busca BB-8, R2-D2, BB-9E y R2-Q5. Despierta al droide sacándolo del cargador.")
        }
        if (app.entries.isNotEmpty()) {
            Panel("Conectados") {
                for (e in app.entries) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Box(Modifier.size(10.dp).clip(CircleShape).background(linkColor(e.droid.link)))
                        Column(Modifier.weight(1f)) {
                            Text("${e.droid.name} · ${e.droid.kind.title}", fontWeight = FontWeight.SemiBold)
                            Hint(e.droid.status)
                        }
                        Chip("Desconectar") { app.disconnect(e) }
                    }
                }
            }
        }
        Panel("Encontrados") {
            if (app.found.isEmpty()) Hint(if (app.scanning) "Buscando droides cercanos…" else "Nada todavía. Pulsa «Buscar droides».")
            for (f in app.found) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    SignalBars(f.rssi.toFloat())
                    Column(Modifier.weight(1f)) {
                        Text(f.name, fontWeight = FontWeight.SemiBold)
                        Hint("${f.kind.title} · ${f.rssi} dBm")
                    }
                    if (app.entries.any { it.droid.id == f.address && it.droid.link != Link.DISCONNECTED }) {
                        Hint("Conectado")
                    } else {
                        Chip("Conectar", fill = Th.accent) {
                            app.connect(f)
                            onClose?.invoke()
                        }
                    }
                }
            }
        }
    }
}

private fun linkColor(link: Link) = when (link) {
    Link.READY -> Th.green
    Link.CONNECTING -> Th.amber
    Link.DISCONNECTED -> Th.muted
}

// ---------------------------------------------------------------------------- pantalla principal

/// Pestañas y subpantallas; mismos grupos que la barra lateral de escritorio.
private val TABS = listOf(
    "Conducir" to emptyList(),
    "Droide" to listOf("Emociones", "Sonidos", "Luces"),
    "Automático" to listOf("Modos", "Seguir", "Patrulla", "Dibujar"),
    "Sistema" to listOf("Mando", "Ajustes", "Registro"),
)

@Composable
private fun MainScreen(app: AppState, e: Entry, onAdd: () -> Unit) {
    var tab by rememberSaveable { mutableStateOf(0) }
    val pages = remember { mutableStateMapOf<Int, Int>() }
    val d = e.droid
    Column(Modifier.fillMaxSize()) {
        // Arriba: los droides conectados.
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 14.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            for (other in app.entries) {
                val selected = other === e
                Row(
                    Modifier.clip(CircleShape).background(if (selected) Th.accentSoft else Th.widget)
                        .border(1.dp, if (selected) Th.accent else Color.Transparent, CircleShape)
                        .clickable { app.activeId = other.droid.id }.padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Box(Modifier.size(9.dp).clip(CircleShape).background(linkColor(other.droid.link)))
                    Text(other.droid.kind.title, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                    Text(other.droid.battery.text, fontSize = 13.sp, color = Th.muted)
                }
            }
            Box(Modifier.clip(CircleShape).background(Th.widget).clickable(onClick = onAdd).padding(horizontal = 12.dp, vertical = 8.dp)) {
                Text("+ Añadir", fontSize = 14.sp)
            }
        }

        // Parada siempre a mano y lo que está en marcha.
        Column(Modifier.fillMaxWidth().background(Th.panel).padding(horizontal = 14.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(
                    Modifier.height(46.dp).clip(RoundedCornerShape(12.dp)).background(Th.red).clickable { e.stopAll(now()) }.padding(horizontal = 18.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text("PARAR TODO", color = Color.White, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                }
                Text("${d.name}: ${d.status}", Modifier.weight(1f), color = Th.muted, fontSize = 13.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            val running = buildList<Pair<String, () -> Unit>> {
                if (e.emoteLoop.running == LoopMode.WWM) add("Viendo una película" to { e.emoteLoop.stop() })
                if (e.emoteLoop.running == LoopMode.EMOTIONS) add("Haciendo monerías" to { e.emoteLoop.stop() })
                if (e.follow.running) add("Siguiéndote" to { e.follow.stop(d) })
                if (e.patrol.running) add("Patrullando" to { e.patrol.stop(d) })
                if (e.draw.running) add("Recorrido" to { e.draw.abort(d) })
                if (d.chatter) add("Charla aleatoria" to { d.setChatterOn(false, now()) })
            }
            if (running.isNotEmpty()) {
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for ((title, stop) in running) {
                        Box(Modifier.clip(CircleShape).background(Th.accentSoft).clickable(onClick = stop).padding(horizontal = 12.dp, vertical = 7.dp)) {
                            Text("$title  ✕", fontSize = 13.sp)
                        }
                    }
                }
            }
        }

        val subs = TABS[tab].second
        val page = pages[tab] ?: 0
        if (subs.isNotEmpty()) SegmentBar(subs, page) { pages[tab] = it }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (tab to page) {
                0 to 0 -> DrivePage(e)
                1 to 0 -> EmotesPage(e)
                1 to 1 -> SoundsPage(e)
                1 to 2 -> LightsPage(e)
                2 to 0 -> AutoPage(e)
                2 to 1 -> FollowPage(app, e)
                2 to 2 -> PatrolPage(e)
                2 to 3 -> DrawPage(e)
                3 to 0 -> PadPage(app, e)
                3 to 1 -> SettingsPage(app, e)
                else -> LogPage(e)
            }
        }

        Row(Modifier.fillMaxWidth().background(Th.panel)) {
            TABS.forEachIndexed { i, (name, _) ->
                Column(Modifier.weight(1f).clickable { tab = i }, horizontalAlignment = Alignment.CenterHorizontally) {
                    Box(Modifier.fillMaxWidth().height(3.dp).background(if (i == tab) Th.accent else Color.Transparent))
                    Text(
                        name, Modifier.padding(vertical = 14.dp), fontSize = 14.sp,
                        fontWeight = if (i == tab) FontWeight.SemiBold else FontWeight.Normal, color = if (i == tab) Th.accent else Th.text,
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------- conducir

@Composable
private fun DrivePage(e: Entry) {
    val d = e.droid
    DisposableEffect(e) {
        onDispose {
            if (e.stick != null) {
                e.stick = null
                d.stop(e.heading)
            }
        }
    }
    Screen {
        Panel {
            if (e.padAiming) Hint("Modo apuntado (mando): orienta el droide con el stick izquierdo y vuelve a pulsarlo para fijar «adelante».", Th.amber)
            Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Joystick(e.heading, onMove = { x, y -> e.stick = x to y }, onEnd = {
                    e.stick = null
                    d.stop(e.heading)
                })
                Text("Rumbo ${e.heading}°", fontWeight = FontWeight.SemiBold)
                Hint("Arrastra el círculo para mover el droide.")
            }
        }
        Panel("Velocidad") {
            LabeledSlider("Máxima: ${(e.maxSpeed * 100).toInt()} %", e.maxSpeed, 0.15f..1f) { e.maxSpeed = it }
            ToggleRow("Turbo", e.turbo) { e.turbo = it }
            ToggleRow("Marcha atrás", e.reverse) { e.reverse = it }
            ToggleRow("Estabilización", e.stabilize) {
                e.stabilize = it
                d.setStabilization(it)
            }
            if (!d.kind.usesV2) Chip("Impulso") { d.boostV1(e.heading) }
        }
        if (d.kind.hasDome) {
            Panel("Patas") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (a in LegAction.entries) Chip(a.title, Modifier.weight(1f)) { d.legAction(a) }
                }
            }
            Panel("Cúpula") {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Slider(value = e.dome, onValueChange = {
                        e.dome = it
                        d.setDome(it)
                    }, valueRange = -160f..180f, modifier = Modifier.weight(1f))
                    Text("${e.dome.toInt()}°", fontSize = 14.sp)
                    Chip("Centrar") {
                        e.dome = 0f
                        d.setDome(0f)
                    }
                }
            }
        }
        Panel("Apuntar") {
            var aiming by remember(e) { mutableStateOf(false) }
            var aimHeading by remember(e) { mutableStateOf(0f) }
            if (aiming) {
                Text("Gira el droide hasta que mire hacia donde quieres que sea «adelante».", fontSize = 14.sp)
                Slider(value = aimHeading, onValueChange = {
                    aimHeading = it
                    d.aim(it.toInt())
                }, valueRange = 0f..359f)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Chip("Fijar como adelante", fill = Th.accent) {
                        d.setForward()
                        d.setAimLight(false)
                        aiming = false
                    }
                    Chip("Cancelar") {
                        d.setAimLight(false)
                        aiming = false
                    }
                }
            } else {
                Hint("Si «adelante» no coincide con lo que ves, reoriéntalo.")
                Chip("Cambiar «adelante»") {
                    aiming = true
                    aimHeading = 0f
                    d.setAimLight(true)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------- emociones, sonidos y luces

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EmotesPage(e: Entry) {
    val d = e.droid
    val anims = d.kind.animations
    Screen {
        if (anims.isEmpty()) {
            Panel { Text("Las animaciones de BB-8 son programas internos de la app original y no están incluidas.") }
            return@Screen
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Chip("Parar animación") { d.stopAnimation() }
            if (d.busy) Pill(Th.amber, "Reproduciendo…")
        }
        for (title in listOf("Emociones", "Reposo", "Patrulla", "Reacciones", "Reacciones (Watch With Me)")) {
            val items = anims.filter { it.categoryTitle == title }
            if (items.isEmpty()) continue
            Panel("$title · ${items.size}") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (a in items) Chip(Spanish.title(a.name), enabled = !d.busy) { d.playAnimation(a, now()) }
                }
            }
        }
    }
}

/// Orden natural: "Hit_2" antes que "Hit_10".
private fun naturalKey(s: String): Pair<String, Int> {
    val digits = s.takeLastWhile { it.isDigit() }
    return s.dropLast(digits.length) to (digits.toIntOrNull() ?: 0)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SoundsPage(e: Entry) {
    val d = e.droid
    var volume by remember(e) { mutableStateOf(150f) }
    var search by remember(e) { mutableStateOf("") }
    val open = remember(e) { mutableStateMapOf<String, Boolean>() }
    Screen {
        if (!d.kind.hasAudio) {
            Panel { Text("BB-8 reproducía sus sonidos desde el teléfono con el motor de audio de la app original, que no está incluido.") }
            return@Screen
        }
        Panel("Audio") {
            LabeledSlider("Volumen: ${volume.toInt()}", volume, 0f..255f, onDone = { d.setVolume(volume.toInt()) }) { volume = it }
            ToggleRow("Charla aleatoria", d.chatter) { d.setChatterOn(it, now()) }
            Chip("Parar audio") { d.stopAudio() }
        }
        Panel("Sonidos") {
            OutlinedTextField(search, { search = it }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text("Buscar sonido…") })
            if (d.kind == Kind.R2Q5) Hint("El R2-Q5 usa aquí la lista de sonidos del R2-D2; puede que alguno no suene.")
            val groups = remember(d.kind, search) {
                d.kind.sounds.filter { search.isBlank() || it.title.contains(search.trim(), ignoreCase = true) }
                    .groupBy { it.groupTitle }.toSortedMap().mapValues { (_, v) -> v.sortedWith(compareBy({ naturalKey(it.name).first }, { naturalKey(it.name).second })) }
            }
            if (groups.isEmpty()) Hint("Ningún sonido coincide.")
            for ((title, items) in groups) {
                val expanded = search.isNotBlank() || open[title] == true
                Text(
                    "${if (expanded) "▾" else "▸"}  $title (${items.size})",
                    Modifier.fillMaxWidth().clickable { open[title] = !expanded }.padding(vertical = 8.dp), fontSize = 15.sp,
                )
                if (expanded) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        for (s in items) Chip(Spanish.title(s.name)) { d.playSound(s) }
                    }
                }
            }
        }
    }
}

private val SWATCHES = listOf(
    Color.White, Color(0xFFFF0000), Color(0xFFFF8000), Color(0xFFFFFF00), Color(0xFF00FF00),
    Color(0xFF00FFFF), Color(0xFF0000FF), Color(0xFFFF00FF), Color.Black,
)

@Composable
private fun LightsPage(e: Entry) {
    val d = e.droid
    val levels = remember(e) { mutableStateMapOf<String, Float>() }
    val chosen = remember(e) { mutableStateMapOf<String, Color>() }
    Screen {
        Panel("Luces") {
            for (ch in d.kind.lights) {
                when (ch.style) {
                    is LightStyle.Rgb, LightStyle.V1Main -> {
                        Text(ch.title, fontSize = 14.sp)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            for (c in SWATCHES) {
                                val selected = chosen[ch.title] == c
                                Box(
                                    Modifier.size(30.dp).clip(CircleShape).background(c)
                                        .border(if (selected) 3.dp else 1.dp, if (selected) Th.accent else Th.border, CircleShape)
                                        .clickable {
                                            chosen[ch.title] = c
                                            d.setLightRgb(ch, (c.red * 255).toInt(), (c.green * 255).toInt(), (c.blue * 255).toInt())
                                        },
                                )
                            }
                        }
                    }
                    is LightStyle.Mono, LightStyle.V1Back -> {
                        val level = levels[ch.title] ?: 0f
                        LabeledSlider("${ch.title}: ${level.toInt()}", level, 0f..255f) {
                            levels[ch.title] = it
                            d.setLightLevel(ch, it.toInt())
                        }
                    }
                }
            }
            Hint("El círculo negro apaga la luz.")
            if (d.kind.usesV2) Chip("Restablecer luces") { d.resetLights() }
        }
    }
}

// ---------------------------------------------------------------------------- automático

@Composable
private fun AutoPage(e: Entry) {
    val d = e.droid
    val loop = e.emoteLoop
    Screen {
        if (d.kind.animations.isEmpty()) {
            Panel { Text("BB-8 no tiene animaciones en esta app.") }
            return@Screen
        }
        for ((mode, title, hint) in listOf(
            Triple(LoopMode.WWM, "Vamos a ver una película", "Reacciones de Watch With Me, al azar."),
            Triple(LoopMode.EMOTIONS, "Haz monerías", "Emotes de Emociones, al azar."),
        )) {
            Panel(title) {
                Hint("$hint ${loop.pool(d.kind, mode).size} emotes.")
                if (mode == LoopMode.WWM) {
                    LabeledSlider("Cada ${loop.wwmInterval.toInt()} s", loop.wwmInterval, 3f..120f) { loop.wwmInterval = it }
                } else {
                    LabeledSlider("Cada ${loop.emotionsInterval.toInt()} s", loop.emotionsInterval, 3f..120f) { loop.emotionsInterval = it }
                    if (d.kind.animations.any { it.isIonBlast }) ToggleRow("Incluir Ion Blast", loop.includeIonBlast) { loop.includeIonBlast = it }
                }
                val on = loop.running == mode
                StartStop(on, "Iniciar", "Apagar") { if (on) loop.stop() else loop.start(mode, now()) }
            }
        }
        if (loop.running != null) Text("Último: ${loop.lastPlayed.ifEmpty { "—" }}", fontSize = 14.sp)
        Hint("Elige un emote al azar cada cierto tiempo; si dura más que el intervalo, espera a que acabe. Mover el droide cancela el emote en curso. Al apagar, el que está sonando termina solo.")
    }
}

@Composable
private fun FollowPage(app: AppState, e: Entry) {
    val d = e.droid
    val f = e.follow
    // Con esta pantalla a la vista se pide la señal aunque el modo esté parado, para verla y calibrar.
    DisposableEffect(e) {
        e.watchSignal = true
        onDispose { e.watchSignal = false }
    }
    val fresh = f.signalFresh(d, app.clock)
    val signal = if (fresh) f.smoothed else null
    Screen {
        Panel("Señal Bluetooth") {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                SignalBars(signal)
                Text(signal?.let { "${Math.round(it)} dBm" } ?: "— dBm", fontSize = 28.sp, fontWeight = FontWeight.Bold)
                if (signal != null) Hint(if (signal >= f.nearThreshold) "cerca" else if (signal >= f.nearThreshold - 12) "a media distancia" else "lejos")
            }
            SignalChart(f.history, Follow.HISTORY, listOf(Triple(f.nearThreshold, Th.green, "cerca"), Triple(Follow.LOST_LEVEL, Th.red, "demasiado lejos")))
            if (!fresh) Hint("No llegan medidas de señal. Sin ellas el droide no se mueve en este modo.", Th.amber)
        }
        Panel("Seguir al móvil") {
            Text(f.status, fontWeight = FontWeight.SemiBold)
            StartStop(f.running, "Empezar a seguir", "Dejar de seguir") { if (f.running) f.stop(d) else f.start(d, now()) }
        }
        Panel("Ajustes") {
            LabeledSlider("Se considera «cerca» a partir de ${f.nearThreshold.toInt()} dBm", f.nearThreshold, -80f..-35f) { f.nearThreshold = Math.round(it).toFloat() }
            Chip("Usar la señal actual como «cerca»", enabled = signal != null) { f.calibrateNear() }
            Hint("Para calibrar, deja el droide a la distancia a la que quieres que se pare y pulsa el botón.")
            LabeledSlider("Velocidad: ${f.speed.toInt()}", f.speed, 30f..120f) { f.speed = it }
        }
        Panel("Cómo funciona") {
            Hint(
                "El droide solo conoce la intensidad de la señal Bluetooth, no la dirección. Avanza zigzagueando, apunta la señal en cada punto " +
                    "del recorrido y con las medidas de los últimos segundos calcula hacia dónde crece; hacia ahí corrige el rumbo. Frena al " +
                    "acercarse, se para si deja de recibir medidas y, si la señal baja de -85 dBm, vuelve sobre sus pasos una vez antes de " +
                    "detenerse. Es aproximado: úsalo en un espacio despejado y vigílalo.",
            )
        }
    }
}

@Composable
private fun PatrolPage(e: Entry) {
    val d = e.droid
    val p = e.patrol
    Screen {
        Panel("Patrulla") {
            Hint("El droide avanza solo, retrocede y gira al chocar, y de vez en cuando lanza una alarma. Déjalo en un espacio despejado.")
            LabeledSlider("Velocidad: ${p.speed.toInt()}", p.speed, 30f..150f) { p.speed = it }
            StartStop(p.running, "Iniciar patrulla", "Detener patrulla") { if (p.running) p.stop(d) else p.start(d, now()) }
        }
        Panel("Registro de eventos") {
            if (p.entries.isEmpty()) Hint("Sin eventos todavía.")
            for (line in p.entries.take(60)) Text(line, fontFamily = FontFamily.Monospace, fontSize = 12.sp)
        }
    }
}

@Composable
private fun DrawPage(e: Entry) {
    val d = e.droid
    val dd = e.draw
    Screen {
        Panel {
            Hint("Dibuja el recorrido con el dedo. El punto verde es la salida y «delante del droide» queda hacia arriba.")
            Canvas(
                Modifier.fillMaxWidth().height(320.dp).clip(RoundedCornerShape(12.dp)).background(Th.well).border(1.dp, Th.border, RoundedCornerShape(12.dp))
                    .pointerInput(e) {
                        // Los puntos se guardan en dp para que la calibración (cm por punto) no dependa de la pantalla.
                        fun add(p: Offset) {
                            if (!dd.running && dd.points.size < 2000) dd.points.add(p.x / density to p.y / density)
                        }
                        awaitEachGesture {
                            val down = awaitFirstDown()
                            add(down.position)
                            while (true) {
                                val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                                if (!change.pressed) break
                                add(change.position)
                                change.consume()
                            }
                        }
                    },
            ) {
                val pts = dd.points.map { Offset(it.first * density, it.second * density) }
                if (pts.size >= 2) {
                    val path = Path()
                    pts.forEachIndexed { i, p -> if (i == 0) path.moveTo(p.x, p.y) else path.lineTo(p.x, p.y) }
                    drawPath(path, Th.accent, style = Stroke(4.dp.toPx()))
                }
                pts.firstOrNull()?.let { drawCircle(Th.green, 7.dp.toPx(), it) }
            }
            LinearProgressIndicator(progress = { dd.progress }, modifier = Modifier.fillMaxWidth())
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                StartStop(dd.running, "Ejecutar recorrido", "Detener", Modifier.weight(1f), enabled = dd.running || dd.points.size >= 2) {
                    if (dd.running) dd.abort(d) else dd.run(d, now())
                }
                Chip("Borrar") { dd.clear(d) }
            }
        }
        Panel("Calibración") {
            Hint("Sin sensores de posición, el recorrido se calcula por tiempo. Ajusta estos valores a tu suelo.")
            LabeledSlider("Velocidad: ${dd.speed.toInt()}", dd.speed, 40f..160f) { dd.speed = it }
            LabeledSlider("Distancia real a esa velocidad: ${dd.cmPerSecond.toInt()} cm/s", dd.cmPerSecond, 10f..100f) { dd.cmPerSecond = it }
            LabeledSlider("Escala del dibujo: ${"%.2f".format(dd.cmPerPoint)} cm/punto", dd.cmPerPoint, 0.1f..1f) { dd.cmPerPoint = it }
        }
    }
}

// ---------------------------------------------------------------------------- sistema

@Composable
private fun PadPage(app: AppState, e: Entry) {
    Screen {
        Panel("Estado") {
            val name = app.pad.name
            if (name != null) Pill(Th.green, "Mando detectado: $name") else {
                Text("Empareja un mando Xbox o PlayStation en los ajustes de Bluetooth del teléfono y pulsa cualquier botón.", fontSize = 14.sp)
            }
            if (e.padAiming) Hint("Modo apuntado activo.", Th.amber)
        }
        Panel("Controles") {
            for ((k, v) in listOf(
                "Stick izquierdo" to "Mueve el droide",
                "Stick derecho" to "Gira la cúpula poco a poco",
                "Clic stick derecho" to "Centra la cúpula",
                "Clic stick izquierdo" to "Apuntar; otra vez para fijar «adelante»",
                "RT / LT" to "Sube / baja la velocidad máxima",
                "A · B · Y · X" to "3 patas · 2 patas · contoneo · parar patas",
                "Start / Options" to "Parar todo",
            )) {
                Column {
                    Text(k, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                    Hint(v)
                }
            }
            Hint("PlayStation: Cruz = A, Círculo = B, Cuadrado = X, Triángulo = Y, R2/L2 = RT/LT. En Android los botones son fijos; no se pueden reasignar todavía.")
        }
    }
}

@Composable
private fun SettingsPage(app: AppState, e: Entry) {
    val d = e.droid
    val kind = d.kind
    Screen {
        Panel("Droide") {
            InfoRow("Nombre", d.name)
            InfoRow("Modelo", kind.title)
            InfoRow("Batería", "${d.battery.text} · ${d.batteryText}")
            if (kind.usesV2) {
                InfoRow("Firmware Nordic", d.versions["Nordic"] ?: "—")
                InfoRow("Firmware principal", d.versions["Principal"] ?: "—")
                InfoRow("Direcciones", d.routingText)
            }
            InfoRow("Firmware en el APK", kind.bundledFirmware)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("Actualizar batería") { d.requestBattery() }
                if (kind.usesV2) Chip("Leer versiones") { d.requestVersions() }
            }
        }
        Panel("Energía") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("Despertar") { d.wake() }
                Chip("Dormir") { d.sleep() }
                if (kind.usesV2) Chip("Sueño profundo") { d.sleep(deep = true) }
            }
            Chip("Desconectar", fill = Th.red) { app.disconnect(e) }
            Hint("En Android la app solo controla el droide mientras está abierta y en pantalla.")
        }
        Panel("Comportamiento") {
            if (kind.usesV2) {
                var interactions by remember(e) { mutableStateOf(Interactions.OFF) }
                Text("Interacciones", fontSize = 14.sp)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (m in Interactions.entries) {
                        Chip(m.title, Modifier.weight(1f), fill = if (m == interactions) Th.accent else Th.widget) {
                            interactions = m
                            d.setInteractions(m)
                        }
                    }
                }
                ToggleRow("Modo trofeo", d.trophyMode) { d.setTrophyModeOn(it) }
                if (kind.hasDome) ToggleRow("La cúpula sigue al movimiento", d.headTracking) { d.setHeadTrackingOn(it) }
            }
            ToggleRow("Reaccionar a los golpes", d.reactToHits) { d.reactToHits = it }
        }
        if (kind.usesV2) {
            Panel("Renombrar") {
                var newName by remember(e) { mutableStateOf("") }
                val valid = Regex("^${kind.prefix}-[A-Z0-9]{4}$").matches(newName)
                OutlinedTextField(newName, { newName = it.uppercase() }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text("${kind.prefix}-XXXX") })
                Chip("Cambiar nombre Bluetooth", enabled = valid) { d.rename(newName) }
                Hint("Debe conservar el prefijo ${kind.prefix}- y tener 4 letras o números; si no, la app no volverá a encontrarlo.")
            }
        }
        Hint(
            "No incluido: actualización de firmware (OTA), realidad aumentada, tienda, Force Band y sincronía de Watch With Me con películas. " +
                "En Android tampoco hay todavía órdenes de voz ni funcionamiento con la pantalla apagada.",
        )
    }
}

@Composable
private fun LogPage(e: Entry) {
    val d = e.droid
    val clipboard = LocalClipboardManager.current
    Column(Modifier.fillMaxSize().padding(horizontal = 14.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Chip("Copiar", fill = Th.accent) {
                val head = "Droide: ${d.name} (${d.kind.title})\nDirecciones: ${d.routingText}\nVersiones: ${d.versions.toMap()}\nBatería: ${d.battery.text} ${d.batteryText}\n---\n"
                clipboard.setText(AnnotatedString(head + d.log.joinToString("\n")))
            }
            Chip("Batería") { d.requestBattery() }
            Chip("Despertar") { d.wake() }
            Chip("Borrar") { d.log.clear() }
        }
        LazyColumn(
            Modifier.weight(1f).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Th.well).border(1.dp, Th.border, RoundedCornerShape(12.dp)).padding(10.dp),
        ) {
            if (d.log.isEmpty()) item { Hint("Registro vacío.") }
            items(d.log) { line -> Text(line, fontFamily = FontFamily.Monospace, fontSize = 11.sp, lineHeight = 15.sp) }
        }
    }
}
