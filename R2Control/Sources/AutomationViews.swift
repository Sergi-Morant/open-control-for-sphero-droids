import SwiftUI

struct PatrolView: View {
    @ObservedObject var droid: Droid
    @StateObject private var patrol = PatrolController()

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading) {
                    Text("Velocidad: \(Int(patrol.speed))")
                    Slider(value: $patrol.speed, in: 30...150)
                }
                Button(patrol.running ? "Detener patrulla" : "Iniciar patrulla") {
                    if patrol.running { patrol.stop() } else { patrol.start(droid) }
                }
                .buttonStyle(.borderedProminent)
                .tint(patrol.running ? .red : .accentColor)
            } footer: {
                Text("El droide avanza solo, retrocede y gira al chocar, y de vez en cuando lanza una alarma. Déjalo en un espacio despejado.")
            }
            Section("Registro de eventos") {
                ForEach(patrol.entries) { e in
                    HStack { Text(e.time, style: .time).foregroundStyle(.secondary); Text(e.text) }.font(.callout)
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
            Canvas { ctx, size in
                var path = Path()
                if let first = dd.points.first {
                    path.move(to: first)
                    dd.points.dropFirst().forEach { path.addLine(to: $0) }
                }
                ctx.stroke(path, with: .color(.accentColor), lineWidth: 4)
                if let first = dd.points.first {
                    ctx.fill(Path(ellipseIn: CGRect(x: first.x - 7, y: first.y - 7, width: 14, height: 14)), with: .color(.green))
                }
            }
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
            .overlay(alignment: .top) { Text("▲ delante del droide").font(.caption2).foregroundStyle(.secondary).padding(4) }
            .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                if !dd.running, dd.points.count < 2000 { dd.points.append(v.location) }
            })

            ProgressView(value: dd.progress)

            HStack {
                Button(dd.running ? "Detener" : "Ejecutar recorrido") {
                    if dd.running { dd.abort() } else { dd.run(droid) }
                }
                .buttonStyle(.borderedProminent).tint(dd.running ? .red : .accentColor)
                .disabled(dd.points.count < 2 && !dd.running)
                Button("Borrar", action: dd.clear).buttonStyle(.bordered)
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
        .padding()
        .onDisappear { dd.abort() }
    }
}

struct LightsView: View {
    @ObservedObject var droid: Droid
    @State private var colors: [String: Color] = [:]
    @State private var levels: [String: Double] = [:]

    var body: some View {
        Form {
            ForEach(droid.kind.lights) { ch in
                switch ch.style {
                case .rgb, .v1Main:
                    ColorPicker(ch.title, selection: Binding(
                        get: { colors[ch.id] ?? .blue },
                        set: { colors[ch.id] = $0; let (r, g, b) = rgb($0); droid.setLight(ch, r: r, g: g, b: b) }))
                case .mono, .v1Back:
                    VStack(alignment: .leading) {
                        Text(ch.title)
                        Slider(value: Binding(
                            get: { levels[ch.id] ?? 0 },
                            set: { levels[ch.id] = $0; droid.setLight(ch, level: UInt8($0)) }), in: 0...255)
                    }
                }
            }
            if droid.kind.usesV2 {
                Button("Restablecer luces", action: droid.resetLights)
            }
        }
    }

    private func rgb(_ c: Color) -> (UInt8, UInt8, UInt8) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(c).getRed(&r, green: &g, blue: &b, alpha: &a)
        return (UInt8(r * 255), UInt8(g * 255), UInt8(b * 255))
    }
}

struct AutoEmotesView: View {
    @ObservedObject var droid: Droid
    @StateObject private var loop = EmoteLoopController()

    private var hasIonBlast: Bool { droid.kind.animations.contains(where: EmoteLoopController.isIonBlast) }

    var body: some View {
        if droid.kind.animations.isEmpty {
            ContentUnavailableView("Sin animaciones", systemImage: "face.dashed",
                                   description: Text("BB-8 no tiene animaciones en esta app."))
        } else {
            Form {
                Section {
                    interval($loop.wwmInterval)
                    startButton(.wwm)
                } header: { Text("Reacciones (Watch With Me) · \(loop.pool(droid, .wwm).count) emotes") }

                Section {
                    interval($loop.emotionsInterval)
                    if hasIonBlast { Toggle("Incluir Ion Blast", isOn: $loop.includeIonBlast) }
                    startButton(.emotions)
                } header: { Text("Emociones · \(loop.pool(droid, .emotions).count) emotes") }

                Section {
                    if loop.running != nil { Label("Último: \(loop.lastPlayed.isEmpty ? "—" : loop.lastPlayed)", systemImage: "play.circle") }
                } footer: {
                    Text("Elige un emote al azar cada cierto tiempo. Si el emote dura más que el intervalo, espera a que acabe. Mover el joystick cancela el emote en curso. Al apagar, el emote que está sonando termina solo.")
                }
            }
        }
    }

    private func interval(_ value: Binding<Double>) -> some View {
        VStack(alignment: .leading) {
            Text("Cada \(Int(value.wrappedValue)) s")
            Slider(value: value, in: 3...120, step: 1)
        }
    }

    private func startButton(_ mode: EmoteLoopController.Mode) -> some View {
        let on = loop.running == mode
        return Button(on ? "Apagar" : "Iniciar") {
            if on { loop.stop() } else { loop.start(mode, on: droid) }
        }
        .buttonStyle(.borderedProminent)
        .tint(on ? .red : .accentColor)
    }
}
