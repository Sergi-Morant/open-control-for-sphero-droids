import SwiftUI

/// Aspecto de la app: los mismos colores y piezas que la versión de escritorio (desktop/src/theme.rs).
enum Theme {
    static let bg = Color(red: 15 / 255, green: 18 / 255, blue: 24 / 255)
    static let panel = Color(red: 20 / 255, green: 24 / 255, blue: 32 / 255)
    static let card = Color(red: 28 / 255, green: 33 / 255, blue: 44 / 255)
    static let border = Color(red: 44 / 255, green: 52 / 255, blue: 68 / 255)
    static let widget = Color(red: 42 / 255, green: 50 / 255, blue: 66 / 255)
    static let well = Color(red: 10 / 255, green: 12 / 255, blue: 16 / 255)
    static let muted = Color(red: 140 / 255, green: 150 / 255, blue: 170 / 255)
    static let accent = Color(red: 64 / 255, green: 140 / 255, blue: 240 / 255)
    static let green = Color(red: 76 / 255, green: 195 / 255, blue: 138 / 255)
    static let amber = Color(red: 238 / 255, green: 176 / 255, blue: 72 / 255)
    static let red = Color(red: 214 / 255, green: 72 / 255, blue: 66 / 255)

    /// Color orientativo para una intensidad de señal en dBm.
    static func signalColor(_ dbm: Double) -> Color {
        dbm >= -60 ? green : dbm >= -75 ? amber : red
    }
}

/// Pantalla desplazable con el fondo de la app; dentro van las tarjetas.
struct Screen<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) { content }
                .padding(.horizontal, 14).padding(.vertical, 12)
        }
        .background(Theme.bg)
    }
}

/// Tarjeta con título opcional que ocupa todo el ancho.
struct Card<Content: View>: View {
    var title: String?
    @ViewBuilder var content: Content

    init(_ title: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let title { Text(title).font(.headline) }
            content
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.border, lineWidth: 1))
    }
}

/// Texto secundario de ayuda.
struct Hint: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.footnote).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
    }
}

/// Botón grande de arrancar/parar un modo.
struct StartStopButton: View {
    let on: Bool
    let start: String
    let stop: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(on ? stop : start).font(.body.weight(.semibold)).foregroundStyle(.white)
                .frame(maxWidth: .infinity, minHeight: 48)
                .background(on ? Theme.red : Theme.accent, in: RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }
}

/// Botón secundario relleno (acciones sueltas, emotes, sonidos).
struct ChipButtonStyle: ButtonStyle {
    var fill = Theme.widget
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.callout)
            .padding(.horizontal, 12).padding(.vertical, 9)
            .background(configuration.isPressed ? Theme.accent : fill, in: RoundedRectangle(cornerRadius: 10))
            .opacity(enabled ? 1 : 0.45)
    }
}

/// Etiqueta redondeada con un punto de color.
struct Pill: View {
    let color: Color
    let text: String

    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(color).frame(width: 8, height: 8)
            Text(text).font(.footnote)
        }
        .padding(.horizontal, 10).padding(.vertical, 5)
        .background(Theme.panel, in: Capsule())
        .overlay(Capsule().stroke(Theme.border, lineWidth: 1))
    }
}

/// Cuatro barras de cobertura.
struct SignalBars: View {
    let dbm: Double?

    var body: some View {
        let lit = dbm.map { Int(min(max(($0 + 92) / 9, 0), 4).rounded(.up)) } ?? 0
        let color = dbm.map(Theme.signalColor) ?? Theme.border
        HStack(alignment: .bottom, spacing: 2) {
            ForEach(0..<4, id: \.self) { i in
                RoundedRectangle(cornerRadius: 1.5).fill(i < lit ? color : Theme.border)
                    .frame(width: 5, height: 5 + CGFloat(i) * 4)
            }
        }
        .frame(height: 18, alignment: .bottom)
    }
}

/// Gráfica sencilla de una serie en dBm con líneas de referencia.
struct SignalChart: View {
    struct Mark { let value: Double; let color: Color; let label: String }
    let values: [Double]
    let capacity: Int
    let marks: [Mark]

    var body: some View {
        Canvas { ctx, size in
            let (lo, hi) = (-95.0, -30.0)
            func y(_ v: Double) -> CGFloat {
                size.height - 8 - CGFloat((min(max(v, lo), hi) - lo) / (hi - lo)) * (size.height - 16)
            }
            for m in marks {
                var line = Path()
                line.move(to: CGPoint(x: 8, y: y(m.value)))
                line.addLine(to: CGPoint(x: size.width - 8, y: y(m.value)))
                ctx.stroke(line, with: .color(m.color.opacity(0.7)), lineWidth: 1)
                ctx.draw(Text(m.label).font(.caption2).foregroundColor(m.color),
                         at: CGPoint(x: size.width - 10, y: y(m.value) - 2), anchor: .bottomTrailing)
            }
            guard values.count >= 2 else {
                ctx.draw(Text("Sin medidas todavía").font(.footnote).foregroundColor(Theme.muted),
                         at: CGPoint(x: size.width / 2, y: size.height / 2))
                return
            }
            let step = (size.width - 16) / CGFloat(max(capacity, 2) - 1)
            let x0 = size.width - 8 - step * CGFloat(values.count - 1)
            var path = Path()
            for (i, v) in values.enumerated() {
                let p = CGPoint(x: x0 + step * CGFloat(i), y: y(v))
                if i == 0 { path.move(to: p) } else { path.addLine(to: p) }
            }
            ctx.stroke(path, with: .color(Theme.accent), lineWidth: 2)
            let last = CGPoint(x: size.width - 8, y: y(values[values.count - 1]))
            ctx.fill(Path(ellipseIn: CGRect(x: last.x - 4, y: last.y - 4, width: 8, height: 8)), with: .color(Theme.accent))
        }
        .frame(height: 140)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.border, lineWidth: 1))
    }
}

/// Coloca los elementos en filas y salta de línea cuando no caben (para rejillas de botones de ancho variable).
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let rows = arrange(subviews, width: width)
        return CGSize(width: proposal.width ?? rows.width, height: rows.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let placed = arrange(subviews, width: bounds.width)
        for (i, p) in placed.points.enumerated() {
            subviews[i].place(at: CGPoint(x: bounds.minX + p.x, y: bounds.minY + p.y),
                              proposal: ProposedViewSize(width: min(placed.sizes[i].width, bounds.width), height: nil))
        }
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> (points: [CGPoint], sizes: [CGSize], width: CGFloat, height: CGFloat) {
        var points: [CGPoint] = [], sizes: [CGSize] = []
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0, maxX: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(ProposedViewSize(width: width.isFinite ? width : nil, height: nil))
            if x > 0, x + size.width > width {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            points.append(CGPoint(x: x, y: y))
            sizes.append(size)
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
            maxX = max(maxX, x - spacing)
        }
        return (points, sizes, maxX, y + rowHeight)
    }
}

/// Selector de subpantalla en la parte superior de una pestaña.
struct SegmentBar<T: Hashable & CaseIterable & Identifiable>: View where T.AllCases: RandomAccessCollection {
    @Binding var selection: T
    let title: (T) -> String

    var body: some View {
        Picker("", selection: $selection) {
            ForEach(T.allCases) { Text(title($0)).tag($0) }
        }
        .pickerStyle(.segmented)
        .padding(.horizontal, 14).padding(.vertical, 8)
        .background(Theme.bg)
    }
}

extension View {
    /// Listas y formularios con el fondo y las filas del tema.
    func themedList() -> some View {
        scrollContentBackground(.hidden).background(Theme.bg)
    }
}
