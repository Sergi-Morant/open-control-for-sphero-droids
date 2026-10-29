import CoreBluetooth
import Foundation

/// Busca droides de todos los tipos y mantiene varias conexiones a la vez.
final class DroidManager: NSObject, ObservableObject {
    struct Found: Identifiable {
        let id: UUID
        let peripheral: CBPeripheral
        let name: String
        let kind: DroidKind
        var rssi: Int
    }

    @Published var found: [Found] = []
    @Published var scanning = false
    @Published var bluetoothText = "Iniciando Bluetooth…"
    @Published var droids: [Droid] = []
    @Published var activeID: UUID?

    private var central: CBCentralManager!

    var active: Droid? { droids.first { $0.id == activeID } ?? droids.first }
    var readyDroids: [Droid] { droids.filter(\.isReady) }

    override init() {
        super.init()
        central = CBCentralManager(delegate: self, queue: .main)
    }

    func startScan() {
        guard central.state == .poweredOn else { bluetoothText = "Activa el Bluetooth en Ajustes"; return }
        found = []
        scanning = true
        central.scanForPeripherals(withServices: nil, options: nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 20) { [weak self] in self?.stopScan() }
    }

    func stopScan() {
        central.stopScan()
        scanning = false
    }

    func connect(_ f: Found) {
        stopScan()
        if let existing = droids.first(where: { $0.id == f.id }) {
            if existing.link == .disconnected { existing.link = .connecting; central.connect(f.peripheral) }
            return
        }
        let d = Droid(kind: f.kind, peripheral: f.peripheral, name: f.name)
        droids.append(d)
        if activeID == nil { activeID = d.id }
        central.connect(f.peripheral)
    }

    func disconnect(_ d: Droid) {
        central.cancelPeripheralConnection(d.peripheral)
        droids.removeAll { $0.id == d.id }
        if activeID == d.id { activeID = droids.first?.id }
    }
}

extension DroidManager: CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ c: CBCentralManager) {
        switch c.state {
        case .poweredOn: bluetoothText = "Bluetooth listo"
        case .unauthorized: bluetoothText = "Permiso de Bluetooth denegado (Ajustes › R2 Control)"
        case .poweredOff: bluetoothText = "Bluetooth apagado"
        default: bluetoothText = "Bluetooth no disponible"
        }
    }

    func centralManager(_ c: CBCentralManager, didDiscover p: CBPeripheral,
                        advertisementData: [String: Any], rssi RSSI: NSNumber) {
        let name = (advertisementData[CBAdvertisementDataLocalNameKey] as? String) ?? p.name ?? ""
        guard let kind = DroidKind.from(bleName: name) else { return }
        if let i = found.firstIndex(where: { $0.id == p.identifier }) {
            found[i].rssi = RSSI.intValue
        } else {
            found.append(Found(id: p.identifier, peripheral: p, name: name, kind: kind, rssi: RSSI.intValue))
        }
    }

    func centralManager(_ c: CBCentralManager, didConnect p: CBPeripheral) {
        droids.first { $0.id == p.identifier }?.didConnect()
    }

    func centralManager(_ c: CBCentralManager, didFailToConnect p: CBPeripheral, error: Error?) {
        guard let d = droids.first(where: { $0.id == p.identifier }) else { return }
        d.link = .disconnected
        d.status = "No se pudo conectar: \(error?.localizedDescription ?? "?")"
    }

    func centralManager(_ c: CBCentralManager, didDisconnectPeripheral p: CBPeripheral, error: Error?) {
        droids.first { $0.id == p.identifier }?.didDisconnect()
    }
}
