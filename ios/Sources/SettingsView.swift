import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var manager: DroidManager
    @ObservedObject var droid: Droid
    @State private var interactions = InteractionMode.off
    @State private var newName = ""
    @State private var confirmRename = false

    /// Versiones de firmware incluidas en el APK original (FirmwareVersionDefinitions.csv).
    private var bundledFirmware: String {
        switch droid.kind {
        case .bb8: return "ST 4.69"
        case .r2d2: return "Nordic 7.0.101 · ST 7.1.86"
        case .bb9e: return "Nordic 7.0.68 · ST 7.0.62"
        case .r2q5: return "Nordic 7.0.102 · ST 7.1.87"
        }
    }

    private var validName: Bool {
        newName.range(of: "^\(droid.kind.prefix)-[A-Z0-9]{4}$", options: .regularExpression) != nil
    }

    var body: some View {
        Form {
          Group {
            Section("Droide") {
                LabeledContent("Nombre", value: droid.name)
                LabeledContent("Modelo", value: droid.kind.title)
                LabeledContent("Batería") { Label("\(droid.batteryLevel.rawValue) · \(droid.batteryText)", systemImage: droid.batteryLevel.symbol) }
                Button("Actualizar batería", action: droid.requestBattery)
                if droid.kind.usesV2 {
                    LabeledContent("Firmware Nordic", value: droid.versions["Nordic"] ?? "—")
                    LabeledContent("Firmware principal", value: droid.versions["Principal"] ?? "—")
                    Button("Leer versiones", action: droid.requestVersions)
                    LabeledContent("Direcciones", value: droid.routingText)
                }
                LabeledContent("Firmware en el APK", value: bundledFirmware)
            }

            if droid.kind.usesV2 {
                Section("Comportamiento autónomo") {
                    Picker("Interacciones", selection: $interactions) {
                        ForEach(InteractionMode.allCases) { Text($0.title).tag($0) }
                    }
                    .onChange(of: interactions) { _, m in droid.setInteractions(m) }
                    Toggle("Modo trofeo", isOn: Binding(get: { droid.trophyMode }, set: { droid.setTrophyMode($0) }))
                    if droid.kind.hasDome {
                        Toggle("La cúpula sigue al movimiento", isOn: Binding(get: { droid.headTracking }, set: { droid.setHeadTracking($0) }))
                    }
                }
            }

            Section("Reacciones") {
                Toggle("Reaccionar a los golpes", isOn: $droid.reactToHits)
            }

            if droid.kind.usesV2 {
                Section {
                    TextField("\(droid.kind.prefix)-XXXX", text: $newName).textInputAutocapitalization(.characters)
                    Button("Cambiar nombre Bluetooth") { confirmRename = true }.disabled(!validName)
                } header: { Text("Renombrar") } footer: {
                    Text("Debe conservar el prefijo \(droid.kind.prefix)- y tener 4 letras o números; si no, la app no volverá a encontrarlo.")
                }
                .confirmationDialog("¿Renombrar a \(newName)?", isPresented: $confirmRename) {
                    Button("Renombrar") { droid.rename(to: newName) }
                }
            }

            Section("No incluido") {
                Text("Actualización de firmware (OTA), realidad aumentada, tienda, Force Band y sincronía de Watch With Me con películas: dependen de servicios o recursos de la app original, o pueden dejar el droide inservible si fallan.")
                    .font(.footnote).foregroundStyle(.secondary)
            }
          }
          .listRowBackground(Theme.card)
        }
        .themedList()
    }
}

struct EnergyView: View {
    @EnvironmentObject var manager: DroidManager
    @ObservedObject var droid: Droid
    @ObservedObject private var keeper = BackgroundKeeper.shared

    var body: some View {
        Form {
          Group {
            Section("Energía") {
                Button("Despertar", action: droid.wake)
                Button("Dormir") { droid.sleep() }
                if droid.kind.usesV2 { Button("Sueño profundo") { droid.sleep(deep: true) } }
                Button("Desconectar", role: .destructive) { manager.disconnect(droid) }
            }

            Section {
                Toggle("Seguir activo con la pantalla bloqueada", isOn: $keeper.keepAlive)
                if keeper.keepAlive {
                    LabeledContent("Estado", value: keeper.running ? "Activo" : "Detenido")
                }
                Toggle("No bloquear la pantalla automáticamente", isOn: $keeper.keepScreenOn)
            } header: { Text("Segundo plano") } footer: {
                Text("Está activado por defecto. Con el iPhone bloqueado la app sigue enviando órdenes al droide (mando, modos automáticos, voz y seguimiento) porque reproduce una señal inaudible para que iOS no la suspenda. Gasta más batería y el iPhone mostrará el indicador de audio. Si el estado dice «Detenido», desactívalo y vuelve a activarlo.")
            }
          }
          .listRowBackground(Theme.card)
        }
        .themedList()
    }
}

struct LogView: View {
    @ObservedObject var droid: Droid

    private var report: String {
        let head = ["Droide: \(droid.name) (\(droid.kind.title))", "Direcciones: \(droid.routingText)",
                    "Versiones: \(droid.versions)", "Batería: \(droid.batteryLevel.rawValue) \(droid.batteryText)", "---"]
        return (head + droid.log).joined(separator: "\n")
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Button("Copiar") { UIPasteboard.general.string = report }
                    .buttonStyle(ChipButtonStyle(fill: Theme.accent)).foregroundStyle(.white)
                Button("Batería", action: droid.requestBattery).buttonStyle(ChipButtonStyle())
                Button("Despertar", action: droid.wake).buttonStyle(ChipButtonStyle())
                Button("Borrar") { droid.log = [] }.buttonStyle(ChipButtonStyle())
            }
            .padding(.vertical, 8)
            ScrollView {
                Text(droid.log.isEmpty ? "Registro vacío." : droid.log.joined(separator: "\n"))
                    .font(.system(.caption2, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
            }
            .background(Theme.well, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.border, lineWidth: 1))
            .padding([.horizontal, .bottom], 14)
        }
        .background(Theme.bg)
    }
}
