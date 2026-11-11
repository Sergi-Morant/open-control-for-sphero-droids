import SwiftUI

struct EmotesView: View {
    @ObservedObject var droid: Droid

    private var groups: [(String, [DroidAnimation])] {
        let order = ["Emociones", "Reposo", "Patrulla", "Reacciones", "Reacciones (Watch With Me)"]
        let dict = Dictionary(grouping: droid.kind.animations, by: \.categoryTitle)
        return dict.sorted { (order.firstIndex(of: $0.key) ?? 99) < (order.firstIndex(of: $1.key) ?? 99) }
    }

    var body: some View {
        if droid.kind.animations.isEmpty {
            ContentUnavailableView("Sin animaciones", systemImage: "face.dashed",
                                   description: Text("Las animaciones de BB-8 son programas internos de la app original (assets de Unity) y no están incluidas."))
                .background(Theme.bg)
        } else {
            Screen {
                HStack(spacing: 10) {
                    Button("Parar animación", action: droid.stopAnimation).buttonStyle(ChipButtonStyle())
                    if droid.busy { Pill(color: Theme.amber, text: "Reproduciendo…") }
                }
                ForEach(groups, id: \.0) { title, items in
                    Card("\(title) · \(items.count)") {
                        FlowLayout {
                            ForEach(items) { a in
                                Button(a.title) { droid.play(a) }.buttonStyle(ChipButtonStyle()).disabled(droid.busy)
                            }
                        }
                    }
                }
            }
        }
    }
}

struct SoundsView: View {
    @ObservedObject var droid: Droid
    @State private var volume = 150.0
    @State private var search = ""

    private var groups: [(String, [DroidSound])] {
        let filtered = droid.kind.sounds.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search) }
        return Dictionary(grouping: filtered, by: \.groupTitle).sorted { $0.key.localizedStandardCompare($1.key) == .orderedAscending }
            .map { ($0.key, $0.value.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }) }
    }

    var body: some View {
        if !droid.kind.hasAudio {
            ContentUnavailableView("Sin altavoz", systemImage: "speaker.slash",
                                   description: Text("BB-8 reproducía sus sonidos desde el teléfono con el motor Wwise de la app original, que no está incluido."))
                .background(Theme.bg)
        } else {
            // Lista (y no tarjetas) porque son cientos de sonidos y así solo se crean las filas visibles.
            List {
                Section("Audio") {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Volumen: \(Int(volume))").font(.callout)
                        Slider(value: $volume, in: 0...255) { editing in if !editing { droid.setVolume(UInt8(volume)) } }
                    }
                    Toggle("Charla aleatoria", isOn: Binding(get: { droid.chatter }, set: { droid.setChatter($0) }))
                    Button("Parar audio", role: .destructive, action: droid.stopAudio)
                }
                .listRowBackground(Theme.card)
                Section {
                    TextField("Buscar sonido…", text: $search).textInputAutocapitalization(.never).autocorrectionDisabled()
                } footer: {
                    if droid.kind == .r2q5 { Text("El R2-Q5 usa aquí la lista de sonidos del R2-D2; puede que alguno no suene.") }
                }
                .listRowBackground(Theme.card)
                ForEach(groups, id: \.0) { title, items in
                    Section("\(title) (\(items.count))") {
                        ForEach(items) { s in Button(s.title) { droid.play(s) }.foregroundStyle(.primary) }
                    }
                    .listRowBackground(Theme.card)
                }
            }
            .themedList()
        }
    }
}
