// Translation for Quick Open: Apple's own, on this Mac (macOS 26 translates
// outside SwiftUI), and NaturalLanguage's guess at what a text is in.
// Swift, as Translation is; built by build.rs into a static library.

import Foundation
import NaturalLanguage
import Translation

private func owned(_ s: String) -> UnsafeMutablePointer<CChar>? {
    strdup(s)
}

/// What `text` is in (a BCP 47 code: "es", "zh-Hans"), as NaturalLanguage
/// guesses (well, unhinted: "hola" is Spanish): malloc'd, or NULL.
@_cdecl("nf_language_of")
public func nf_language_of(_ text: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    let recognizer = NLLanguageRecognizer()
    recognizer.processString(String(cString: text))
    return recognizer.dominantLanguage.flatMap { owned($0.rawValue) }
}

/// Holds the answer for the thread that waits on it.
private final class Outcome: @unchecked Sendable {
    private let lock = NSLock()
    private var value = "timed-out"
    func set(_ v: String) { lock.lock(); value = v; lock.unlock() }
    func get() -> String { lock.lock(); defer { lock.unlock() }; return value }
}

/// `text` from `from` into `to` (BCP 47 codes), on this Mac: "=" and the
/// translation, or why not: "unsupported" (Apple doesn't translate between
/// them), "not-installed" (its languages aren't downloaded), "old-macos",
/// "timed-out", or the error. Waits, up to `seconds`: not on the main
/// thread, where Translation answers. malloc'd.
@_cdecl("nf_translate")
public func nf_translate(_ text: UnsafePointer<CChar>, _ from: UnsafePointer<CChar>, _ to: UnsafePointer<CChar>, _ seconds: Double) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *) else { return owned("old-macos") }
    let text = String(cString: text)
    let source = Locale.Language(identifier: String(cString: from))
    let target = Locale.Language(identifier: String(cString: to))
    let outcome = Outcome()
    let done = DispatchSemaphore(value: 0)
    Task.detached {
        switch await LanguageAvailability().status(from: source, to: target) {
        case .installed:
            do {
                let session = TranslationSession(installedSource: source, target: target)
                outcome.set("=" + (try await session.translate(text)).targetText)
            } catch {
                outcome.set(error.localizedDescription)
            }
        case .supported:
            outcome.set("not-installed")
        default:
            outcome.set("unsupported")
        }
        done.signal()
    }
    _ = done.wait(timeout: .now() + seconds)
    return owned(outcome.get())
}
