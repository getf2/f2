// Dictation's speech to text: Apple's own (SpeechAnalyzer and
// SpeechTranscriber, macOS 26), on this Mac, each language's model
// downloaded once by macOS (AssetInventory), not kept in F2. Given the
// audio dictation.m heard (16 kHz, mono), it says what was said. Swift, as
// SpeechAnalyzer is; built by build.rs with translate.swift.
//
// After Hex's src/apple_speech.swift (https://github.com/anomalyco/hex),
// under its MIT License:
//
// Copyright (c) 2026 Christopher Langton
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

import AVFAudio
import Foundation
import Speech

private func owned(_ s: String) -> UnsafeMutablePointer<CChar>? {
    strdup(s)
}

private struct Failure: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

/// Holds the answer for the thread that waits on it.
private final class Waited<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Result<Value, Error>?
    func set(_ v: Result<Value, Error>) { lock.lock(); value = v; lock.unlock() }
    func get() -> Result<Value, Error>? { lock.lock(); defer { lock.unlock() }; return value }
}

/// `work`'s answer, waited for up to `seconds`: not on the main thread.
private func waiting<Value: Sendable>(_ seconds: Double, _ what: String, _ work: @escaping @Sendable () async throws -> Value) -> Result<Value, Error> {
    let done = DispatchSemaphore(value: 0)
    let answer = Waited<Value>()
    let task = Task.detached {
        do {
            answer.set(.success(try await work()))
        } catch {
            answer.set(.failure(error))
        }
        done.signal()
    }
    guard done.wait(timeout: .now() + seconds) == .success, let got = answer.get() else {
        task.cancel()
        return .failure(Failure("\(what) took too long."))
    }
    return got
}

private func message(_ error: Error) -> String {
    (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
}

/// The audio it's given: as dictation.m makes it. (SpeechAnalyzer takes
/// another, 16-bit: given this as it is, it stops the app. See `best`.)
private let format = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false)!

/// How far the model's download has got (0–1; -1: none going).
private final class Download: @unchecked Sendable {
    private let lock = NSLock()
    private var current: Progress?
    func set(_ p: Progress?) { lock.lock(); current = p; lock.unlock() }
    func fraction() -> Double { lock.lock(); defer { lock.unlock() }; return current.map { $0.fractionCompleted } ?? -1 }
}

private let downloading = Download()

@available(macOS 26.0, *)
private actor Speech {
    static let shared = Speech()

    /// A transcriber made ready while the key's held (its language's).
    private var warm: (String, SpeechTranscriber, SpeechAnalyzer)?
    private var reserved: Set<String> = []

    private func make(_ locale: Locale) -> (SpeechTranscriber, SpeechAnalyzer) {
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
        let analyzer = SpeechAnalyzer(modules: [transcriber], options: SpeechAnalyzer.Options(priority: .userInitiated, modelRetention: .lingering))
        return (transcriber, analyzer)
    }

    func installed(_ locale: Locale) async -> Bool {
        await AssetInventory.status(forModules: [SpeechTranscriber(locale: locale, preset: .transcription)]) == .installed
    }

    /// Its model downloaded (macOS keeps it, reserved for F2).
    func prepare(_ locale: Locale) async throws {
        let id = locale.identifier(.bcp47)
        if !reserved.contains(id) {
            // (macOS keeps a few languages reserved for each app: one that
            // can't be still downloads.)
            if (try? await AssetInventory.reserve(locale: locale)) == true {
                reserved.insert(id)
            }
        }
        let transcriber = SpeechTranscriber(locale: locale, preset: .transcription)
        if await AssetInventory.status(forModules: [transcriber]) != .installed,
           let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            downloading.set(request.progress)
            defer { downloading.set(nil) }
            try await request.downloadAndInstall()
        }
        guard await AssetInventory.status(forModules: [transcriber]) == .installed else {
            throw Failure("macOS didn't download the speech model for \(name(id)).")
        }
    }

    /// The audio format `transcriber` takes.
    private func best(_ transcriber: SpeechTranscriber) async -> AVAudioFormat {
        await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) ?? format
    }

    func warmUp(_ locale: Locale) async {
        let id = locale.identifier(.bcp47)
        guard warm?.0 != id, await installed(locale) else { return }
        let (transcriber, analyzer) = make(locale)
        try? await analyzer.prepareToAnalyze(in: await best(transcriber))
        warm = (id, transcriber, analyzer)
    }

    func transcribe(_ samples: [Float], _ locale: Locale) async throws -> String {
        let id = locale.identifier(.bcp47)
        let transcriber: SpeechTranscriber
        let analyzer: SpeechAnalyzer
        if let ready = warm, ready.0 == id {
            (transcriber, analyzer) = (ready.1, ready.2)
        } else {
            (transcriber, analyzer) = make(locale)
        }
        warm = nil
        guard let heard = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)), let channel = heard.floatChannelData?[0] else {
            throw Failure("No room for the audio.")
        }
        heard.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { channel.update(from: $0.baseAddress!, count: samples.count) }
        let buffer = try convert(heard, to: await best(transcriber))
        let input = AsyncStream<AnalyzerInput> { more in
            more.yield(AnalyzerInput(buffer: buffer))
            more.finish()
        }
        let said = Task {
            var text = AttributedString()
            for try await result in transcriber.results where result.isFinal {
                text += result.text
            }
            return String(text.characters)
        }
        do {
            if let last = try await analyzer.analyzeSequence(input) {
                try await analyzer.finalizeAndFinish(through: last)
            } else {
                await analyzer.cancelAndFinishNow()
            }
            return try await said.value.trimmingCharacters(in: .whitespacesAndNewlines)
        } catch {
            said.cancel()
            await analyzer.cancelAndFinishNow()
            throw error
        }
    }
}

/// `buffer` in `to`'s format (as it is, if it's that already).
private func convert(_ buffer: AVAudioPCMBuffer, to: AVAudioFormat) throws -> AVAudioPCMBuffer {
    if buffer.format == to { return buffer }
    guard let converter = AVAudioConverter(from: buffer.format, to: to),
          let out = AVAudioPCMBuffer(pcmFormat: to, frameCapacity: AVAudioFrameCount(Double(buffer.frameLength) * to.sampleRate / buffer.format.sampleRate) + 1024) else {
        throw Failure("Couldn't convert the audio.")
    }
    var given = false
    var error: NSError?
    converter.convert(to: out, error: &error) { _, status in
        if given {
            status.pointee = .endOfStream
            return nil
        }
        given = true
        status.pointee = .haveData
        return buffer
    }
    if let error { throw error }
    return out
}

/// A language's name, in English (F2's).
private func name(_ id: String) -> String {
    Locale(identifier: "en_US").localizedString(forIdentifier: id) ?? id
}

/// Whether this Mac turns speech into text (macOS 26, and a Mac it runs on).
@_cdecl("nf_speech_available")
public func nf_speech_available() -> Bool {
    guard #available(macOS 26.0, *) else { return false }
    return SpeechTranscriber.isAvailable
}

/// The languages it takes: a line each, its code (BCP 47), a tab, its
/// name, a tab, and "1" if its model's on this Mac. Not on the main
/// thread. malloc'd.
@_cdecl("nf_speech_locales")
public func nf_speech_locales() -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *) else { return nil }
    let got = waiting(10, "Listing languages") { () async -> String in
        let installed = Set(await SpeechTranscriber.installedLocales.map { $0.identifier(.bcp47) })
        return await SpeechTranscriber.supportedLocales
            .map { $0.identifier(.bcp47) }
            .sorted { name($0) < name($1) }
            .map { "\($0)\t\(name($0))\t\(installed.contains($0) ? "1" : "0")" }
            .joined(separator: "\n")
    }
    return (try? got.get()).flatMap(owned)
}

/// The language it'd use for `id` ("": this Mac's): its code, or NULL if
/// it has none like it. Not on the main thread. malloc'd.
@_cdecl("nf_speech_resolve")
public func nf_speech_resolve(_ id: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *) else { return nil }
    let wanted = String(cString: id)
    let got = waiting(10, "Finding the language") { () async -> String? in
        let locale = wanted.isEmpty ? Locale.current : Locale(identifier: wanted)
        return await SpeechTranscriber.supportedLocale(equivalentTo: locale)?.identifier(.bcp47)
    }
    return (try? got.get()).flatMap { $0 }.flatMap(owned)
}

/// Ready for `id`: its model downloaded. Waits (a download, the first
/// time): not on the main thread. NULL, or why not (malloc'd). (Unlike
/// SFSpeechRecognizer, SpeechAnalyzer doesn't need Speech Recognition
/// allowed: it's all on this Mac.)
@_cdecl("nf_speech_prepare")
public func nf_speech_prepare(_ id: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *), SpeechTranscriber.isAvailable else { return owned("Dictation needs macOS 26.") }
    let locale = Locale(identifier: String(cString: id))
    let got = waiting(30 * 60, "Downloading the speech model") { try await Speech.shared.prepare(locale) }
    if case .failure(let error) = got {
        return owned(message(error))
    }
    return nil
}

/// How far a model's download has got (0–1), -1 when there's none.
@_cdecl("nf_speech_progress")
public func nf_speech_progress() -> Double {
    downloading.fraction()
}

/// Make the next transcription in `id` quicker (audio's coming). Returns
/// at once.
@_cdecl("nf_speech_warm")
public func nf_speech_warm(_ id: UnsafePointer<CChar>) {
    guard #available(macOS 26.0, *) else { return }
    let locale = Locale(identifier: String(cString: id))
    Task.detached { await Speech.shared.warmUp(locale) }
}

/// What's said in `samples` (`count` of them, 16 kHz mono), in `id`: the
/// text (malloc'd; maybe empty), or NULL and why in `error`. Waits: not on
/// the main thread.
@_cdecl("nf_speech_transcribe")
public func nf_speech_transcribe(_ id: UnsafePointer<CChar>, _ samples: UnsafePointer<Float>, _ count: Int, _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26.0, *) else {
        error.pointee = owned("Dictation needs macOS 26.")
        return nil
    }
    let locale = Locale(identifier: String(cString: id))
    let audio = Array(UnsafeBufferPointer(start: samples, count: count))
    let got = waiting(120, "Transcribing") { try await Speech.shared.transcribe(audio, locale) }
    switch got {
    case .success(let text):
        return owned(text)
    case .failure(let why):
        error.pointee = owned(message(why))
        return nil
    }
}
