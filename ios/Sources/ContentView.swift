import SwiftUI

struct ContentView: View {
    @EnvironmentObject var manager: DroidManager
    @State private var showConnect = false

    var body: some View {
        NavigationStack {
            Group {
                if manager.droids.isEmpty {
                    ConnectView()
                } else {
                    MainView(showConnect: $showConnect)
                }
            }
            .background(Theme.bg)
            .navigationTitle("Open control for Sphero Droids")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(Theme.panel, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
        }
        .sheet(isPresented: $showConnect) {
            NavigationStack {
                ConnectView()
                    .navigationTitle("Droides")
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { Button("Cerrar") { showConnect = false } }
            }
            .preferredColorScheme(.dark)
            .tint(Theme.accent)
        }
        .preferredColorScheme(.dark)
        .tint(Theme.accent)
    }
}

// MARK: - Conexión

struct ConnectView: View {
    @EnvironmentObject var manager: DroidManager

    var body: some View {
        Screen {
            Card {
                Button(action: manager.startScan) {
                    HStack(spacing: 10) {
                        if manager.scanning { ProgressView().tint(.white) }
                        Text(manager.scanning ? "Buscando…" : "Buscar droides").font(.body.weight(.semibold))
                    }
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity, minHeight: 48)
                    .background(Theme.accent.opacity(manager.scanning ? 0.5 : 1), in: RoundedRectangle(cornerRadius: 12))
                }
                .buttonStyle(.plain)
                .disabled(manager.scanning)
                Text(manager.bluetoothText).font(.footnote)
                Hint("Busca BB-8, R2-D2, BB-9E y R2-Q5. Despierta al droide sacándolo del cargador.")
            }

            if !manager.droids.isEmpty {
                Card("Conectados") {
                    ForEach(manager.droids) { d in ConnectedRow(droid: d) }
                }
            }

            Card("Encontrados") {
                if manager.found.isEmpty {
                    Hint(manager.scanning ? "Buscando droides cercanos…" : "Nada todavía. Pulsa «Buscar droides».")
                }
                ForEach(manager.found) { f in
                    HStack(spacing: 10) {
                        SignalBars(dbm: Double(f.rssi))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(f.name).font(.body.weight(.semibold))
                            Text("\(f.kind.title) · \(f.rssi) dBm").font(.caption).foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        if manager.droids.contains(where: { $0.id == f.id && $0.link != .disconnected }) {
                            Text("Conectado").font(.footnote).foregroundStyle(Theme.muted)
                        } else {
                            Button("Conectar") { manager.connect(f) }
                                .buttonStyle(ChipButtonStyle(fill: Theme.accent)).foregroundStyle(.white)
                        }
                    }
                }
            }
        }
    }
}

struct ConnectedRow: View {
    @EnvironmentObject var manager: DroidManager
    @ObservedObject var droid: Droid

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(droid.isReady ? Theme.green : Theme.amber).frame(width: 10, height: 10)
            VStack(alignment: .leading, spacing: 2) {
                Text("\(droid.name) · \(droid.kind.title)").font(.body.weight(.semibold))
                Text(droid.status).font(.caption).foregroundStyle(Theme.muted)
            }
            Spacer()
            Button("Desconectar") { manager.disconnect(droid) }.buttonStyle(ChipButtonStyle())
        }
    }
}

// MARK: - Pantalla principal

struct MainView: View {
    @EnvironmentObject var manager: DroidManager
    @Binding var showConnect: Bool

    var body: some View {
        VStack(spacing: 0) {
            DroidBar(showConnect: $showConnect)
            if let d = manager.active { DroidTabs(droid: d).id(d.id) }
        }
        .background(Theme.bg)
    }
}

struct DroidBar: View {
    @EnvironmentObject var manager: DroidManager
    @Binding var showConnect: Bool

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack {
                ForEach(manager.droids) { d in DroidChip(droid: d, selected: d.id == manager.active?.id) { manager.activeID = d.id } }
                Button { showConnect = true } label: {
                    Label("Añadir", systemImage: "plus").font(.callout)
                        .padding(.horizontal, 12).padding(.vertical, 7)
                        .background(Theme.widget, in: Capsule())
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 14).padding(.vertical, 8)
        }
    }
}

struct DroidChip: View {
    @ObservedObject var droid: Droid
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Circle().fill(droid.isReady ? Theme.green : Theme.amber).frame(width: 9, height: 9)
                Text(droid.kind.title).font(.callout.weight(.semibold))
                Image(systemName: droid.batteryLevel.symbol).foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, 12).padding(.vertical, 7)
            .background(selected ? Theme.accent.opacity(0.35) : Theme.widget, in: Capsule())
            .overlay(Capsule().stroke(selected ? Theme.accent : .clear, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }
}

/// Subpantallas de cada pestaña; mismos grupos que la barra lateral de la app de escritorio.
enum DroidPage: String, CaseIterable, Identifiable { case emotes = "Emociones", sounds = "Sonidos", lights = "Luces"; var id: String { rawValue } }
enum AutoPage: String, CaseIterable, Identifiable { case modes = "Modos", follow = "Seguir", patrol = "Patrulla", draw = "Dibujar"; var id: String { rawValue } }
enum SystemPage: String, CaseIterable, Identifiable { case gamepad = "Mando", settings = "Ajustes", energy = "Energía", log = "Registro"; var id: String { rawValue } }

struct DroidTabs: View {
    @ObservedObject var droid: Droid
    @StateObject private var loop = EmoteLoopController()
    @StateObject private var follow = FollowController()
    @StateObject private var voice = VoiceController()
    @StateObject private var pad = GamepadController()
    @State private var droidPage = DroidPage.emotes
    @State private var autoPage = AutoPage.modes
    @State private var systemPage = SystemPage.gamepad

    /// Detiene todo lo que la app haya puesto en marcha (también lo iniciado por voz).
    private func stopAll() {
        loop.stop()
        follow.stop()
        droid.setChatter(false)
        droid.stopAnimation()
        droid.stopAudio()
        droid.stop()
    }

    var body: some View {
        VStack(spacing: 0) {
            VoiceBar(voice: voice, droid: droid, loop: loop, follow: follow, stopAll: stopAll)
            if !droid.isReady {
                Text(droid.status).font(.footnote).foregroundStyle(Theme.amber)
                    .padding(6).frame(maxWidth: .infinity).background(Theme.amber.opacity(0.15))
            }
            TabView {
                DriveView(droid: droid, pad: pad)
                    .tabItem { Label("Conducir", systemImage: "gamecontroller") }

                VStack(spacing: 0) {
                    SegmentBar(selection: $droidPage) { $0.rawValue }
                    switch droidPage {
                    case .emotes: EmotesView(droid: droid)
                    case .sounds: SoundsView(droid: droid)
                    case .lights: LightsView(droid: droid)
                    }
                }
                .tabItem { Label("Droide", systemImage: "face.smiling") }

                VStack(spacing: 0) {
                    SegmentBar(selection: $autoPage) { $0.rawValue }
                    switch autoPage {
                    case .modes: AutoEmotesView(droid: droid, loop: loop)
                    case .follow: FollowView(droid: droid, follow: follow)
                    case .patrol: PatrolView(droid: droid)
                    case .draw: DrawDriveView(droid: droid)
                    }
                }
                .tabItem { Label("Automático", systemImage: "repeat") }

                VStack(spacing: 0) {
                    SegmentBar(selection: $systemPage) { $0.rawValue }
                    switch systemPage {
                    case .gamepad: GamepadView(pad: pad)
                    case .settings: SettingsView(droid: droid)
                    case .energy: EnergyView(droid: droid)
                    case .log: LogView(droid: droid)
                    }
                }
                .tabItem { Label("Sistema", systemImage: "gearshape") }
            }
            .toolbarBackground(Theme.panel, for: .tabBar)
            .toolbarBackground(.visible, for: .tabBar)
        }
        .background(Theme.bg)
        .onAppear {
            voice.configure(droid: droid, loop: loop, follow: follow)
            follow.attach(droid)
            pad.start(droid: droid, loop: loop, stopAll: stopAll)
            BackgroundKeeper.shared.resume()
        }
        .onDisappear { voice.stop(); loop.stop(); follow.stop(); pad.stop() }
    }
}
