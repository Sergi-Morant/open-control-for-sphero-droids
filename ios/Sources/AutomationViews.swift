import SwiftUI

struct PatrolView: View {
    @ObservedObject var droid: Droid
    @StateObject private var patrol = PatrolController()

    var body: some View {
        Screen {
            Card("Patrulla") {
                Hint("El droide avanza solo, retrocede y gira al chocar, y de vez en cuando lanza una alarma. Déjalo en un espacio despejado.")
                VStack(alignment: .leading, spacing: 4) {
                    Text("Velocidad: \(Int(patrol.speed))").font(.callout)
                    Slider(value: $patrol.speed, in: 30...150)
                }
                StartStopButton(on: patrol.running, start: "Iniciar patrulla", stop: "Detener patrulla") {
                    if patrol.running { patrol.stop() } else { patrol.start(droid) }
                }
            }
            Card("Registro de eventos") {
                if patrol.entries.isEmpty { Hint("Sin eventos todavía.") }
                ForEach(patrol.entries) { e in
                    HStack(alignment: .top) { Text(e.time, style: .time).foregroundStyle(Theme.muted); Text(e.text) }.font(.callout)
                }
            }
        }
        .onDisappear { patrol.stop() }
    }
}

struct DrawDriveView: View {
    @ObservedObject var droid: Droid
    @StateObject private var dd = DrawDriveController()

    var body: some View {
        VStack(spacing: 12) {
            Hint("Dibuja el recorrido con el dedo. El punto verde es la salida.")
                .frame(maxWidth: .infinity, alignment: .leading)
            Canvas { ctx, size in
                var path = Path()
                if let first = dd.points.first {
                    path.move(to: first)
                    dd.points.dropFirst().forEach { path.addLine(to: $0) }
                }
                ctx.stroke(path, with: .color(Theme.accent), lineWidth: 4)
                if let first = dd.points.first {
                    ctx.fill(Path(ellipseIn: CGRect(x: first.x - 7, y: first.y - 7, width: 14, height: 14)), with: .color(Theme.green))
                }
            }
            .background(Theme.well, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.border, lineWidth: 1))
            .overlay(alignment: .top) { Text("▲ delante del droide").font(.caption2).foregroundStyle(Theme.muted).padding(6) }
            .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                if !dd.running, dd.points.count < 2000 { dd.points.append(v.location) }
            })

            ProgressView(value: dd.progress)

            HStack(spacing: 10) {
                StartStopButton(on: dd.running, start: "Ejecutar recorrido", stop: "Detener") {
                    if dd.running { dd.abort() } else { dd.run(droid) }
                }
                .disabled(dd.points.count < 2 && !dd.running)
                .opacity(dd.points.count < 2 && !dd.running ? 0.45 : 1)
                Button("Borrar", action: dd.clear).buttonStyle(ChipButtonStyle())
            }

            DisclosureGroup("Calibración") {
                VStack(alignment: .leading) {
                    Text("Velocidad: \(Int(dd.speed))"); Slider(value: $dd.speed, in: 40...160)
                    Text("Distancia real a esa velocidad: \(Int(dd.cmPerSecond)) cm/s"); Slider(value: $dd.cmPerSecond, in: 10...100)
                    Text("Escala del dibujo: \(String(format: "%.2f", dd.cmPerPoint)) cm/punto"); Slider(value: $dd.cmPerPoint, in: 0.1...1)
                }
                .font(.footnote)
            }
        }
        .padding(14)
        .background(Theme.bg)
        .onDisappear { dd.abort() }
    }
}

struct LightsView: View {
    @ObservedObject var droid: Droid
    @State private var colors: [String: Color] = [:]
    @State private var levels: [String: Double] = [:]

    var body: some View {
        Screen {
            Card("Luces") {
                ForEach(droid.kind.lights) { ch in
                    switch ch.style {
                    case .rgb, .v1Main:
                        ColorPicker(ch.title, selection: Binding(
                            get: { colors[ch.id] ?? .blue },
                            set: { colors[ch.id] = $0; let (r, g, b) = rgb($0); droid.setLight(ch, r: r, g: g, b: b) }))
                    case .mono, .v1Back:
                        VStack(alignment: .leading, spacing: 4) {
                            Text("\(ch.title): \(Int(levels[ch.id] ?? 0))").font(.callout)
                            Slider(value: Binding(
                                get: { levels[ch.id] ?? 0 },
                                set: { levels[ch.id] = $0; droid.setLight(ch, level: UInt8($0)) }), in: 0...255)
                        }
                    }
                }
                if droid.kind.usesV2 {
                    Button("Restablecer luces", action: droid.resetLights).buttonStyle(ChipButtonStyle())
                }
            }
        }
    }

    private func rgb(_ c: Color) -> (UInt8, UInt8, UInt8) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(c).getRed(&r, green: &g, blue: &b, alpha: &a)
        return (UInt8(max(0, min(1, r)) * 255), UInt8(max(0, min(1, g)) * 255), UInt8(max(0, min(1, b)) * 255))
    }
}

/// Los dos modos automáticos de emotes.
struct AutoEmotesView: View {
    @ObservedObject var droid: Droid
    @ObservedObject var loop: EmoteLoopController

    private var hasIonBlast: Bool { droid.kind.animations.contains(where: EmoteLoopController.isIonBlast) }

    var body: some View {
        if droid.kind.animations.isEmpty {
            ContentUnavailableView("Sin animaciones", systemImage: "face.dashed",
                                   description: Text("BB-8 no tiene animaciones en esta app."))
                .background(Theme.bg)
        } else {
            Screen {
                Card("Vamos a ver una película") {
                    Hint("Reacciones de Watch With Me, al azar. \(loop.pool(droid, .wwm).count) emotes.")
                    interval($loop.wwmInterval)
                    startButton(.wwm)
                }
                Card("Haz monerías") {
                    Hint("Emotes de Emociones, al azar. \(loop.pool(droid, .emotions).count) emotes.")
                    interval($loop.emotionsInterval)
                    if hasIonBlast { Toggle("Incluir Ion Blast", isOn: $loop.includeIonBlast) }
                    startButton(.emotions)
                }
                if loop.running != nil {
                    Label("Último: \(loop.lastPlayed.isEmpty ? "—" : loop.lastPlayed)", systemImage: "play.circle").font(.callout)
                }
                Hint("También puedes activarlos por voz con el micrófono. Elige un emote al azar cada cierto tiempo; si dura más que el intervalo, espera a que acabe. Mover el joystick cancela el emote en curso. Al apagar, el que está sonando termina solo.")
            }
        }
    }

    private func interval(_ value: Binding<Double>) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Cada \(Int(value.wrappedValue)) s").font(.callout)
            Slider(value: value, in: 3...120, step: 1)
        }
    }

    private func startButton(_ mode: EmoteLoopController.Mode) -> some View {
        let on = loop.running == mode
        return StartStopButton(on: on, start: "Iniciar", stop: "Apagar") {
            if on { loop.stop() } else { loop.start(mode, on: droid) }
        }
    }
}

struct FollowView: View {
    @ObservedObject var droid: Droid
    @ObservedObject var follow: FollowController
    @State private var now = Date()
    private let clock = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    var body: some View {
        let fresh = follow.isFresh(at: now)
        let signal = fresh ? follow.smoothed : nil
        Screen {
            Card("Señal Bluetooth") {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    SignalBars(dbm: signal)
                    Text(signal.map { "\(Int($0.rounded())) dBm" } ?? "— dBm").font(.system(size: 28, weight: .bold))
                    if let s = signal {
                        Text(s >= follow.nearThreshold ? "cerca" : s >= follow.nearThreshold - 12 ? "a media distancia" : "lejos")
                            .font(.callout).foregroundStyle(Theme.muted)
                    }
                }
                SignalChart(values: follow.history, capacity: FollowController.historySize, marks: [
                    .init(value: follow.nearThreshold, color: Theme.green, label: "cerca"),
                    .init(value: -85, color: Theme.red, label: "demasiado lejos"),
                ])
                if !fresh {
                    Text("No llegan medidas de señal. Sin ellas el droide no se mueve en este modo.")
                        .font(.footnote).foregroundStyle(Theme.amber)
                }
            }
            Card("Seguir al móvil") {
                Text(follow.status).font(.body.weight(.semibold))
                StartStopButton(on: follow.running, start: "Empezar a seguir", stop: "Dejar de seguir") {
                    if follow.running { follow.stop() } else { follow.start(droid) }
                }
            }
            Card("Ajustes") {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Se considera «cerca» a partir de \(Int(follow.nearThreshold)) dBm").font(.callout)
                    Slider(value: $follow.nearThreshold, in: -80 ... -35, step: 1)
                }
                Button("Usar la señal actual como «cerca»", action: follow.calibrateNear)
                    .buttonStyle(ChipButtonStyle()).disabled(signal == nil)
                Hint("Para calibrar, deja el droide a la distancia a la que quieres que se pare y pulsa el botón.")
                VStack(alignment: .leading, spacing: 4) {
                    Text("Velocidad: \(Int(follow.speed))").font(.callout)
                    Slider(value: $follow.speed, in: 30...120)
                }
            }
            Card("Cómo funciona") {
                Hint("El droide solo conoce la intensidad de la señal Bluetooth, no la dirección. Avanza zigzagueando, apunta la señal en cada punto del recorrido y con las medidas de los últimos segundos calcula hacia dónde crece; hacia ahí corrige el rumbo. Frena al acercarse, se para si deja de recibir medidas y, si la señal baja de -85 dBm, vuelve sobre sus pasos una vez antes de detenerse. Es aproximado: úsalo en un espacio despejado y vigílalo. Al chocar, rectifica.")
            }
        }
        .onReceive(clock) { t in
            now = t
            // Con el modo parado, esta pantalla pide la señal para poder verla y calibrar.
            if !follow.running { droid.readRSSI() }
        }
    }
}

struct VoiceBar: View {
    @ObservedObject var voice: VoiceController
    @ObservedObject var droid: Droid
    @ObservedObject var loop: EmoteLoopController
    @ObservedObject var follow: FollowController
    let stopAll: () -> Void

    var body: some View {
        VStack(spacing: 8) {
            HStack(spacing: 10) {
                Button { voice.toggle() } label: {
                    Image(systemName: voice.listening ? "mic.fill" : "mic.slash")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                        .background(voice.listening ? Theme.red.opacity(0.3) : Theme.widget, in: Circle())
                }
                .buttonStyle(.plain)
                VStack(alignment: .leading, spacing: 1) {
                    if let e = voice.error {
                        Text(e).font(.caption).foregroundStyle(Theme.red)
                    } else if voice.listening {
                        Text(voice.lastAction.isEmpty ? "Escuchando…" : "✓ \(voice.lastAction)").font(.caption.bold())
                        Text(voice.heard).font(.caption2).foregroundStyle(Theme.muted).lineLimit(1)
                    } else {
                        Text("Voz desactivada").font(.caption).foregroundStyle(Theme.muted)
                    }
                }
                Spacer()
                Button(action: stopAll) {
                    Label("PARAR TODO", systemImage: "stop.fill").font(.callout.bold()).foregroundStyle(.white)
                        .padding(.horizontal, 14).frame(height: 44)
                        .background(Theme.red, in: RoundedRectangle(cornerRadius: 12))
                }
                .buttonStyle(.plain)
            }
            if loop.running != nil || follow.running || droid.chatter {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        if loop.running == .wwm { chip("Viendo una película", loop.stop) }
                        if loop.running == .emotions { chip("Haciendo monerías", loop.stop) }
                        if follow.running { chip("Siguiéndote", follow.stop) }
                        if droid.chatter { chip("Charla aleatoria") { droid.setChatter(false) } }
                    }
                }
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 8)
        .background(Theme.panel)
    }

    private func chip(_ title: String, _ stop: @escaping () -> Void) -> some View {
        Button(action: stop) {
            HStack(spacing: 4) { Text(title); Image(systemName: "xmark.circle.fill") }
                .font(.caption)
                .padding(.horizontal, 10).padding(.vertical, 6)
                .background(Theme.accent.opacity(0.3), in: Capsule())
        }
        .buttonStyle(.plain)
    }
}
