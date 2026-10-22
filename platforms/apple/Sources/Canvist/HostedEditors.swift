import Foundation

/// The editors of views a JavaScript host shows, such as React Native or
/// Lynx, by the id the host gave each view.
///
/// A host's JavaScript calls arrive on its own thread while the views run on
/// the main thread. That is safe: ``Editor`` serialises every call, and
/// ``CanvistView`` hears about changes on the main thread wherever they came
/// from.
public final class HostedEditors: @unchecked Sendable {
    /// The registry every host shares.
    public static let shared = HostedEditors()

    private let lock = NSLock()
    /// Weak, so an editor never outlives its view even if a host forgets to
    /// unregister it.
    private var editors: [String: WeakEditor] = [:]

    /// Find `editor` by `id` from now on.
    public func register(_ editor: Editor, id: String) {
        lock.withLock { editors[id] = WeakEditor(editor) }
    }

    /// Forget the editor `id` names.
    public func unregister(id: String) {
        lock.withLock { _ = editors.removeValue(forKey: id) }
    }

    /// Run `body` on the editor `id` names, with every failure as a
    /// ``HostedEditorError``.
    public func call<T>(_ id: String, _ body: (Editor) throws -> T) throws(HostedEditorError) -> T {
        do {
            guard let editor = lock.withLock({ editors[id]?.editor }) else {
                throw HostedEditorError(code: "editor", reason: "no mounted editor has the id `\(id)`")
            }

            return try body(editor)
        } catch {
            throw HostedEditorError(error)
        }
    }
}

/// A failure for a JavaScript host to report, with the stable code every
/// binding shares.
public struct HostedEditorError: Error, Sendable {
    public let code: String
    public let reason: String

    public init(code: String, reason: String) {
        self.code = code
        self.reason = reason
    }

    /// `error` with its code: an engine error keeps its own, anything else
    /// is an `editor` error.
    public init(_ error: any Error) {
        switch error {
        case let error as HostedEditorError: self = error
        case let CanvistError.Failed(code, reason): self.init(code: code, reason: reason)
        default: self.init(code: "editor", reason: error.localizedDescription)
        }
    }

    /// `[canvist:<code>] <reason>`, the form JavaScript reads the code from
    /// (`@canvist/headless/native`).
    public var message: String {
        "[canvist:\(code)] \(reason)"
    }
}

extension DocFormat {
    /// The format a JavaScript name stands for: `json`, `html`, `markdown`,
    /// or `text`.
    public init(name: String) throws(HostedEditorError) {
        switch name {
        case "json": self = .json
        case "html": self = .html
        case "markdown": self = .markdown
        case "text": self = .text
        default: throw HostedEditorError(code: "invalidArgs", reason: "unknown format `\(name)`")
        }
    }
}

extension Editor {
    /// The selection as JSON, `{"anchor": position, "head": position}`
    /// with positions as `{"path": [...], "offset": n}`, or `null` for a
    /// block selection: the `Range` JavaScript reads.
    public func selectionJSON() -> String {
        guard let positions = selectedPositions(), positions.count == 2 else { return "null" }

        return "{\"anchor\":\(positionJSON(positions[0])),\"head\":\(positionJSON(positions[1]))}"
    }

    /// Select the JSON range `json`, as ``selectionJSON()`` writes it.
    /// Returns whether the selection moved.
    public func select(rangeJSON json: String) throws -> Bool {
        struct Position: Decodable {
            let path: [UInt32]
            let offset: UInt32
        }

        struct Range: Decodable {
            let anchor: Position
            let head: Position
        }

        let range: Range

        do {
            range = try JSONDecoder().decode(Range.self, from: Data(json.utf8))
        } catch {
            throw HostedEditorError(code: "invalidPosition", reason: "invalid range \(json): \(error.localizedDescription)")
        }

        return try selectPositions(
            anchor: DocPosition(path: range.anchor.path, offset: range.anchor.offset),
            head: DocPosition(path: range.head.path, offset: range.head.offset)
        )
    }
}

private final class WeakEditor {
    weak var editor: Editor?

    init(_ editor: Editor) {
        self.editor = editor
    }
}

private func positionJSON(_ position: DocPosition) -> String {
    "{\"path\":[\(position.path.map(String.init).joined(separator: ","))],\"offset\":\(position.offset)}"
}
