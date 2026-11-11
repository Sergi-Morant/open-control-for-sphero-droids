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
        Screen {
            Card {
                if pad.aiming {
                    Label("Modo apuntado (mando): orienta el droide con el stick izquierdo y vuelve a pulsarlo para fijar «adelante»", systemImage: "scope")
                        .font(.footnote).foregroundStyle(Theme.amber)
                }
                VStack(spacing: 8) {
                    Joystick(heading: heading) { x, y in
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
                    Text("Rumbo \(heading)°").font(.callout.weight(.semibold))
                    Hint(pad.connectedName == nil ? "Arrastra el círculo para mover el droide." : "También puedes conducir con el mando.")
                }
                .frame(maxWidth: .infinity)
            }

            Card("Velocidad") {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Máxima: \(Int(pad.maxSpeed * 100)) %").font(.callout)
                    Slider(value: $pad.maxSpeed, in: 0.15...1)
                }
                Toggle("Turbo", isOn: $turbo)
                Toggle("Marcha atrás", isOn: $reverse)
                Toggle("Estabilización", isOn: $stabilize).onChange(of: stabilize) { _, v in droid.setStabilization(v) }
                if !droid.kind.usesV2 {
                    Button("Impulso") { droid.boostV1(heading: heading) }.buttonStyle(ChipButtonStyle())
                }
            }

            if droid.kind.hasLegs {
                Card("Patas") {
                    HStack(spacing: 8) {
                        ForEach(LegAction.allCases) { a in
                            Button { droid.legAction(a) } label: { Text(a.title).frame(maxWidth: .infinity) }
                                .buttonStyle(ChipButtonStyle())
                        }
                    }
                }
            }

            if droid.kind.hasDome {
                Card("Cúpula") {
                    HStack(spacing: 12) {
                        Slider(value: Binding(get: { pad.dome }, set: { pad.dome = $0; droid.setDome(Float($0)) }), in: -160...180)
                        Text("\(Int(pad.dome))°").font(.callout.monospacedDigit()).frame(width: 48, alignment: .trailing)
                        Button("Centrar") { pad.dome = 0; droid.setDome(0) }.buttonStyle(ChipButtonStyle())
                    }
                }
            }

            Card("Apuntar") {
                if aiming {
                    Text("Gira el droide hasta que mire hacia donde tú quieres que sea «adelante».").font(.callout)
                    Slider(value: $aimHeading, in: 0...359).onChange(of: aimHeading) { _, v in droid.aim(heading: Int(v)) }
                    HStack(spacing: 8) {
                        Button("Fijar como adelante") { droid.setForward(); setAiming(false) }
                            .buttonStyle(ChipButtonStyle(fill: Theme.accent)).foregroundStyle(.white)
                        Button("Cancelar") { setAiming(false) }.buttonStyle(ChipButtonStyle())
                    }
                } else {
                    Hint("Si «adelante» no coincide con lo que ves, reoriéntalo.")
                    Button("Cambiar «adelante»") { setAiming(true) }.buttonStyle(ChipButtonStyle())
                }
            }
        }
        .onDisappear { stopDriving() }
    }

    private func setAiming(_ on: Bool) {
        aiming = on
        droid.setAimLight(on)
        if on { aimHeading = 0 }
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
    var heading: Int = 0
    var onChange: (CGFloat, CGFloat) -> Void
    var onEnd: () -> Void
    @State private var knob: CGSize = .zero
    private let radius: CGFloat = 105

    var body: some View {
        ZStack {
            Circle().fill(Theme.well)
            Circle().stroke(Theme.border, lineWidth: 2)
            Circle().stroke(Theme.border, lineWidth: 1).frame(width: radius, height: radius)
            Rectangle().fill(Theme.border).frame(width: radius * 2, height: 1)
            Rectangle().fill(Theme.border).frame(width: 1, height: radius * 2)
            // Marca del rumbo en el borde.
            Capsule().fill(Theme.amber).frame(width: 4, height: 14)
                .offset(y: -radius + 7)
                .rotationEffect(.degrees(Double(heading)))
            Circle().fill(knob == .zero ? Theme.accent.opacity(0.85) : Theme.accent).frame(width: 68, height: 68)
                .overlay(Circle().stroke(.white.opacity(0.15), lineWidth: 2))
                .offset(knob)
        }
        .frame(width: radius * 2, height: radius * 2)
        .padding(10)
        .contentShape(Circle())
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
