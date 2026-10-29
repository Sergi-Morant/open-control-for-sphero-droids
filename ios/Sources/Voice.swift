import AVFoundation
import Speech
import SwiftUI

/// Gestiona la sesión de audio y permite que la app siga activa con la pantalla bloqueada.
/// Se usa el modo de audio en segundo plano reproduciendo silencio; sin él, iOS suspende la app al bloquear.
final class BackgroundKeeper: ObservableObject {
    static let shared = BackgroundKeeper()

    @Published var keepAlive = false { didSet { applyKeepAlive() } }
    @Published var keepScreenOn = false { didSet { UIApplication.shared.isIdleTimerDisabled = keepScreenOn } }

    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private var attached = false

    static func activateSession() {
        let s = AVAudioSession.sharedInstance()
        try? s.setCategory(.playAndRecord, mode: .default, options: [.mixWithOthers, .defaultToSpeaker])
        try? s.setActive(true)
    }

    private func applyKeepAlive() {
        if keepAlive {
            Self.activateSession()
            if !attached {
                engine.attach(player)
                let format = AVAudioFormat(standardFormatWithSampleRate: 44100, channels: 1)!
                engine.connect(player, to: engine.mainMixerNode, format: format)
                attached = true
            }
            let format = AVAudioFormat(standardFormatWithSampleRate: 44100, channels: 1)!
            let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 44100)!
            buffer.frameLength = 44100                       // un segundo de silencio
            player.scheduleBuffer(buffer, at: nil, options: .loops)
            try? engine.start()
            player.play()
        } else {
            player.stop()
            engine.stop()
        }
    }
}

/// Reconocimiento de voz con el micrófono del iPhone (el droide no tiene micrófono).
final class VoiceController: NSObject, ObservableObject {
    @Published var listening = false
    @Published var heard = ""
    @Published var lastAction = ""
    @Published var error: String?

    private let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "es-ES"))
    private let engine = AVAudioEngine()
    private let lock = NSLock()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private var generation = 0
    private var handled = 0

    private weak var droid: Droid?
    private var loop: EmoteLoopController?
    private var follow: FollowController?

    func configure(droid: Droid, loop: EmoteLoopController, follow: FollowController) {
        self.droid = droid; self.loop = loop; self.follow = follow
    }

    // MARK: Activar / desactivar

    func toggle() { listening ? stop() : start() }

    func start() {
        error = nil
        SFSpeechRecognizer.requestAuthorization { [weak self] status in
            DispatchQueue.main.async {
                guard status == .authorized else { self?.error = "Permiso de reconocimiento de voz denegado (Ajustes › Open control for Sphero Droids)"; return }
                AVAudioApplication.requestRecordPermission { granted in
                    DispatchQueue.main.async {
                        guard granted else { self?.error = "Permiso de micrófono denegado (Ajustes › Open control for Sphero Droids)"; return }
                        self?.begin()
                    }
                }
            }
        }
    }

    private func begin() {
        guard let recognizer, recognizer.isAvailable else { error = "Reconocimiento de voz no disponible"; return }
        BackgroundKeeper.activateSession()
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            guard let self else { return }
            self.lock.lock(); let r = self.request; self.lock.unlock()
            r?.append(buffer)
        }
        NotificationCenter.default.addObserver(self, selector: #selector(engineChanged),
                                               name: .AVAudioEngineConfigurationChange, object: engine)
        do { try engine.start() } catch { self.error = "No se pudo iniciar el micrófono: \(error.localizedDescription)"; return }
        listening = true
        startTask()
    }

    func stop() {
        listening = false
        generation += 1
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        NotificationCenter.default.removeObserver(self, name: .AVAudioEngineConfigurationChange, object: engine)
        lock.lock(); request?.endAudio(); request = nil; lock.unlock()
        task?.cancel(); task = nil
    }

    @objc private func engineChanged() {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.listening, !self.engine.isRunning else { return }
            try? self.engine.start()
        }
    }

    // MARK: Reconocimiento

    private func startTask() {
        guard listening, let recognizer else { return }
        generation += 1
        let gen = generation
        handled = 0
        let req = SFSpeechAudioBufferRecognitionRequest()
        req.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
        lock.lock(); request = req; lock.unlock()
        task = recognizer.recognitionTask(with: req) { [weak self] result, err in
            DispatchQueue.main.async {
                guard let self, gen == self.generation else { return }
                if let r = result { self.process(r.bestTranscription.formattedString) }
                if err != nil || result?.isFinal == true {
                    // El reconocimiento termina solo cada cierto tiempo: se reinicia para escuchar de forma continua.
                    self.task?.cancel()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { self.startTask() }
                }
            }
        }
    }

    private func process(_ text: String) {
        heard = text
        let norm = text.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "es"))
        if norm.count < handled { handled = 0 }
        let tail = String(norm.dropFirst(handled))
        guard let action = Self.match(tail) else { return }
        handled = norm.count
        run(action)
    }

    private enum Action { case movie, emotes, stop, follow, talk }

    /// Compara sin acentos ni mayúsculas. «Basta/detente/apaga» paran todo.
    private static func match(_ t: String) -> Action? {
        if ["detente", "basta", "apaga", "apagate", "termina", "quieto"].contains(where: t.contains) { return .stop }
        if t.contains("pelicula") && (t.contains("ver") || t.contains("vamos")) { return .movie }
        if t.contains("moneria") || (t.contains("emote") && (t.contains("realiza") || t.contains("haz"))) { return .emotes }
        if ["sigueme", "ven aqui", "ven conmigo"].contains(where: t.contains) { return .follow }
        if ["habla", "di algo"].contains(where: t.contains) { return .talk }
        return nil
    }

    private func run(_ a: Action) {
        guard let d = droid, d.isReady else { lastAction = "El droide no está conectado"; return }
        switch a {
        case .movie:
            follow?.stop(); loop?.start(.wwm, on: d); lastAction = "Vamos a ver una película"
        case .emotes:
            follow?.stop(); loop?.start(.emotions, on: d); lastAction = "Haz monerías"
        case .stop:
            loop?.stop(); follow?.stop(); lastAction = "Parado"
        case .follow:
            loop?.stop(); follow?.start(d); lastAction = "Te sigo"
        case .talk:
            let pool = d.kind.sounds.filter { ["Chatty", "Excited", "Positive", "Hey"].contains($0.group) }
            if let s = pool.randomElement() { d.play(s) }
            lastAction = "Habla"
        }
    }
}
