import SwiftUI

@main
struct R2ControlApp: App {
    @StateObject private var manager = DroidManager()

    var body: some Scene {
        WindowGroup {
            ContentView().environmentObject(manager)
        }
    }
}
