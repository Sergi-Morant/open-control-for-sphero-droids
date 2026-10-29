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
            .navigationTitle("R2 Control")
            .navigationBarTitleDisplayMode(.inline)
        }
        .sheet(isPresented: $showConnect) {
            NavigationStack {
                ConnectView()
                    .navigationTitle("Droides")
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { Button("Cerrar") { showConnect = false } }
            }
        }
    }
}

// MARK: - Conexión

struct ConnectView: View {
    @EnvironmentObject var manager: DroidManager

    var body: some View {
        List {
            Section {
                Text(manager.bluetoothText).foregroundStyle(.secondary)
                Button(manager.scanning ? "Buscando…" : "Buscar droides", action: manager.startScan)
                    .disabled(manager.scanning)
            } footer: {
                Text("Busca BB-8, R2-D2, BB-9E y R2-Q5. Despierta al droide sacándolo del cargador.")
            }

            if !manager.droids.isEmpty {
                Section("Conectados") {
                    ForEach(manager.droids) { d in ConnectedRow(droid: d) }
                }
            }

            Section("Encontrados") {
                ForEach(manager.found) { f in
                    Button { manager.connect(f) } label: {
                        HStack {
                            Image(systemName: f.kind.symbol)
                            VStack(alignment: .leading) { Text(f.name); Text(f.kind.title).font(.caption).foregroundStyle(.secondary) }
                            Spacer()
                            Text("\(f.rssi) dBm").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                if manager.found.isEmpty { Text("Nada todavía").foregroundStyle(.secondary) }
            }
        }
    }
}

struct ConnectedRow: View {
    @EnvironmentObject var manager: DroidManager
    @ObservedObject var droid: Droid

    var body: some View {
        HStack {
            Image(systemName: droid.kind.symbol)
            VStack(alignment: .leading) { Text("\(droid.name) · \(droid.kind.title)"); Text(droid.status).font(.caption).foregroundStyle(.secondary) }
            Spacer()
            Button("Desconectar", role: .destructive) { manager.disconnect(droid) }.font(.caption)
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
    }
}

struct DroidBar: View {
    @EnvironmentObject var manager: DroidManager
    @Binding var showConnect: Bool

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack {
                ForEach(manager.droids) { d in DroidChip(droid: d, selected: d.id == manager.active?.id) { manager.activeID = d.id } }
                Button { showConnect = true } label: { Image(systemName: "plus.circle.fill").font(.title2) }
            }
            .padding(.horizontal).padding(.vertical, 8)
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
                Image(systemName: droid.kind.symbol)
                Text(droid.kind.title)
                Image(systemName: droid.batteryLevel.symbol)
                Circle().fill(droid.isReady ? .green : .orange).frame(width: 8, height: 8)
            }
            .padding(.horizontal, 12).padding(.vertical, 6)
            .background(selected ? Color.accentColor.opacity(0.25) : Color.secondary.opacity(0.15), in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

struct DroidTabs: View {
    @ObservedObject var droid: Droid
    @StateObject private var loop = EmoteLoopController()
    @StateObject private var follow = FollowController()
    @StateObject private var voice = VoiceController()
    @StateObject private var pad = GamepadController()

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
            TabView {
                DriveView(droid: droid, pad: pad).tabItem { Label("Conducir", systemImage: "gamecontroller") }
                EmotesView(droid: droid).tabItem { Label("Emociones", systemImage: "face.smiling") }
                AutoEmotesView(droid: droid, loop: loop).tabItem { Label("Automático", systemImage: "repeat") }
                SoundsView(droid: droid, loop: loop).tabItem { Label("Sonidos", systemImage: "speaker.wave.2") }
                FollowView(droid: droid, follow: follow).tabItem { Label("Seguir", systemImage: "figure.walk") }
                PatrolView(droid: droid).tabItem { Label("Patrulla", systemImage: "binoculars") }
                DrawDriveView(droid: droid).tabItem { Label("Dibujar", systemImage: "scribble") }
                LightsView(droid: droid).tabItem { Label("Luces", systemImage: "lightbulb") }
                GamepadView(pad: pad).tabItem { Label("Mando", systemImage: "gamecontroller.fill") }
                EnergyView(droid: droid).tabItem { Label("Energía", systemImage: "power") }
                SettingsView(droid: droid).tabItem { Label("Ajustes", systemImage: "gearshape") }
                LogView(droid: droid).tabItem { Label("Log", systemImage: "terminal") }
            }
            .overlay(alignment: .top) {
                if !droid.isReady {
                    Text(droid.status).font(.footnote).padding(6).frame(maxWidth: .infinity).background(.orange.opacity(0.3))
                }
            }
        }
        .onAppear {
            voice.configure(droid: droid, loop: loop, follow: follow)
            pad.start(droid: droid, loop: loop, stopAll: stopAll)
        }
        .onDisappear { voice.stop(); loop.stop(); follow.stop(); pad.stop() }
    }
}
