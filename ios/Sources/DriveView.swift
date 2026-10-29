import SwiftUI

struct DriveView: View {
    @ObservedObject var droid: Droid
    @ObservedObject var pad: GamepadController
    @State private var turbo = false
    @State private var reverse = false
    @State private var stabilize = true
    @State private var aiming = false
    @State private var aimHeading = 0.0
    @State private var speed = 0
    @State private var heading = 0
    @State private var driveTimer: Timer?

    var body: some View {
        ScrollView {
            VStack(spacing: 18) {
                if droid.kind.hasLegs {
                    Picker("Patas", selection: Binding<Int>(get: { -1 }, set: { v in
                        if let a = LegAction(rawValue: UInt8(v)) { droid.legAction(a) }
                    })) {
                        Text("Patas").tag(-1)
                        ForEach(LegAction.allCases) { Text($0.title).tag(Int($0.rawValue)) }
                    }
                    .pickerStyle(.segmented)
                }

                Joystick { x, y in
                    let mag = min(1, hypot(x, y))
                    speed = mag > 0.1 ? Int(mag * 255 * (turbo || pad.turbo ? 1 : pad.maxSpeed)) : 0
                    if mag > 0.1 {
                        var deg = atan2(x, -y) * 180 / .pi
                        if deg < 0 { deg += 360 }
                        heading = Int(deg) % 360
                    }
                    startDriving()
                } onEnd: {
                    speed = 0
                    droid.stop(heading: heading)
                    stopDriving()
                }

                HStack {
                    Toggle("Turbo", isOn: $turbo)
                    Toggle("Marcha atrás", isOn: $reverse)
                }
                if !droid.kind.usesV2 {
                    Button("Impulso (boost)") { droid.boostV1(heading: heading) }.buttonStyle(.bordered)
                }
                VStack(alignment: .leading) {
                    Text("Velocidad máxima: \(Int(pad.maxSpeed * 100)) %")
                    Slider(value: $pad.maxSpeed, in: 0.15...1)
                }
                Toggle("Estabilización", isOn: $stabilize).onChange(of: stabilize) { _, v in droid.setStabilization(v) }

                GroupBox("Apuntar") {
                    Toggle("Modo apuntado", isOn: $aiming).onChange(of: aiming) { _, on in
                        droid.setAimLight(on)
                        if on { aimHeading = 0 }
                    }
                    if aiming {
                        Text("Gira el droide hasta que mire hacia donde tú quieres que sea «adelante».").font(.footnote)
                        Slider(value: $aimHeading, in: 0...359).onChange(of: aimHeading) { _, v in droid.aim(heading: Int(v)) }
                        Button("Fijar como adelante") { droid.setForward(); aiming = false }.buttonStyle(.borderedProminent)
                    }
                }

                if droid.kind.hasDome {
                    GroupBox("Cúpula: \(Int(pad.dome))°") {
                        Slider(value: Binding(get: { pad.dome }, set: { pad.dome = $0; droid.setDome(Float($0)) }), in: -160...180)
                    }
                }
            }
            .padding()
        }
        .onDisappear { stopDriving() }
    }

    private func startDriving() {
        guard driveTimer == nil else { return }
        // El droide se detiene si deja de recibir órdenes: se reenvían a ~10 Hz.
        driveTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { _ in
            droid.drive(speed: speed, heading: heading, reverse: reverse)
        }
    }

    private func stopDriving() { driveTimer?.invalidate(); driveTimer = nil }
}

struct Joystick: View {
    var onChange: (CGFloat, CGFloat) -> Void
    var onEnd: () -> Void
    @State private var knob: CGSize = .zero
    private let radius: CGFloat = 90

    var body: some View {
        ZStack {
            Circle().fill(.quaternary).frame(width: radius * 2, height: radius * 2)
            Circle().fill(.tint).frame(width: 64, height: 64).offset(knob)
        }
        .frame(width: radius * 2 + 20, height: radius * 2 + 20)
        .gesture(DragGesture(minimumDistance: 0)
            .onChanged { v in
                let d = hypot(v.translation.width, v.translation.height)
                let k = d > radius ? radius / d : 1
                knob = CGSize(width: v.translation.width * k, height: v.translation.height * k)
                onChange(knob.width / radius, knob.height / radius)
            }
            .onEnded { _ in knob = .zero; onEnd() })
    }
}
