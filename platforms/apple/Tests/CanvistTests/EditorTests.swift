@testable import Canvist
import CoreText
import XCTest

/// The Rust editor through the Swift binding, measuring with CoreText.
final class EditorTests: XCTestCase {
    private let kit = StarterKit()

    private func editor(_ markdown: String = "") throws -> Editor {
        var options = defaultEditorOptions()
        options.extensions = kit.extensions.names
        options.platform = .apple
        let editor = try Editor(options: options, measurer: CoreTextMeasurer())
        try editor.load(format: .markdown, content: markdown)
        editor.setViewport(width: 360, height: 720)

        return editor
    }

    func testTypingRunsInputRules() throws {
        let editor = try editor()

        for character in "## Title" {
            _ = try editor.insertText(text: String(character))
        }

        XCTAssertEqual(editor.export(format: .markdown), "## Title")
        XCTAssertEqual(try editor.query(kit.heading.headingLevel()), 2)
    }

    func testTypedCommandsAndQueries() throws {
        let editor = try editor("Hello world")
        _ = try editor.select(anchor: 6, head: 11)

        XCTAssertTrue(try editor.can(command: kit.bold.toggleBold()))
        XCTAssertTrue(try editor.run(command: kit.bold.toggleBold()))
        XCTAssertTrue(try editor.query(kit.bold.isBold()))
        XCTAssertTrue(try editor.run(command: kit.codeBlock.toggleCodeBlock(language: "swift")))
        XCTAssertEqual(try editor.query(kit.codeBlock.codeBlockLanguage()), "swift")
        XCTAssertNil(try editor.query(kit.link.linkHref()))
    }

    func testInvalidArgumentsThrowTheSharedCode() throws {
        let editor = try editor("text")

        XCTAssertThrowsError(try editor.run(command: kit.heading.toggleHeading(level: 9))) { error in
            guard case CanvistError.Failed(let code, _) = error else {
                return XCTFail("unexpected \(error)")
            }

            XCTAssertEqual(code, "invalidArgs")
        }
    }

    func testCommandArgumentsAreJSON() {
        let link = kit.link.setLink(href: "https://example.com/\"quoted\"\n")
        XCTAssertEqual(link.name, "setLink")
        XCTAssertEqual(link.args, #"{"href":"https://example.com/\"quoted\"\n"}"#)
        XCTAssertEqual(kit.codeBlock.setCodeBlock().args, "{}", "nil arguments are left out")
    }

    func testMarkedTextComposesAndCommits() throws {
        let editor = try editor()

        _ = try editor.setMarkedText(text: "に", selected: OffsetRange(start: 1, end: 1))
        _ = try editor.setMarkedText(text: "にほん", selected: OffsetRange(start: 3, end: 3))
        XCTAssertEqual(editor.markedRange(), OffsetRange(start: 0, end: 3))
        _ = try editor.insertText(text: "日本")

        XCTAssertNil(editor.markedRange())
        XCTAssertEqual(editor.plainText(), "日本")
        XCTAssertEqual(editor.selection(), TextSelection(anchor: 2, head: 2))
    }

    func testFramesPaintTextInTheMeasuredFont() throws {
        let editor = try editor("# Big\n\nsmall")
        let frame = try editor.frame()
        let runs = (frame.items ?? []).compactMap { item -> (String, UInt32)? in
            guard case .text(let text, let font, _, _, _, _, _, _, _) = item else { return nil }
            return (text, font)
        }

        XCTAssertEqual(runs.map(\.0), ["Big", "small"])
        XCTAssertNotEqual(runs[0].1, runs[1].1, "headings use another font")
        XCTAssertGreaterThan(frame.height, 0)
    }

    func testMeasuredAdvancesAddUpToTheLineWidth() {
        let font = CTFontCreateUIFontForLanguage(.system, 17, nil)!
        let samples = [
            "Typography office", "naïve café", "👩🏽‍💻 ok", "مرحبا بالعالم", "漢字かな",
        ]

        for text in samples {
            var advances: [Float] = []
            CoreTextMeasurer.advances(of: text, in: font, into: &advances)
            let line = CTLineCreateWithAttributedString(
                NSAttributedString(string: text, attributes: [kCTFontAttributeName as NSAttributedString.Key: font])
            )
            let width = CTLineGetTypographicBounds(line, nil, nil, nil)

            XCTAssertEqual(advances.count, text.unicodeScalars.count, text)
            XCTAssertEqual(Double(advances.reduce(0, +)), width, accuracy: 0.01, text)
        }
    }

    func testTwoEditorsCollaborate() throws {
        let first = try editor("Hello")
        try first.share(clientId: 1)
        let second = try editor()
        try second.join(state: try first.encodeState(), clientId: 2)

        _ = try first.select(anchor: 5, head: 5)
        _ = try first.insertText(text: " world")
        for update in try first.takeUpdates() {
            _ = try second.applyUpdate(update: update)
        }

        XCTAssertEqual(second.plainText(), "Hello world")
    }

    /// The editor half of keystroke-to-paint through the Swift binding,
    /// CoreText measuring included: type a character and build its frame.
    func testKeystrokeToFrameStaysWithinBudget() throws {
        let paragraph = "The quick brown fox jumps over the lazy dog, again and again."
        let editor = try editor(Array(repeating: paragraph, count: Self.paragraphs).joined(separator: "\n\n"))
        editor.setFocused(focused: true)
        _ = try editor.frame()
        _ = try editor.select(anchor: 10, head: 10)
        _ = try editor.frame()

        let samples = try (0..<Self.keystrokes)
            .map { index in try keystroke(editor, typing: index % 2 == 0) }
            .sorted()
        let median = samples[samples.count / 2]
        let p95 = samples[samples.count * Self.percentile / 100]

        print("PERF swift keystroke-to-frame median=\(Int(median))µs p95=\(Int(p95))µs")
        XCTAssertLessThan(p95, Self.budget * perfSlack, "keystroke-to-frame p95 \(p95)µs is over its budget")
    }

    /// Type a character, or delete one, and build the frame; returns
    /// microseconds.
    private func keystroke(_ editor: Editor, typing: Bool) throws -> Double {
        let start = DispatchTime.now().uptimeNanoseconds
        _ = typing ? try editor.insertText(text: "x") : try editor.deleteBackward()
        _ = try editor.frame()

        return Double(DispatchTime.now().uptimeNanoseconds - start) / Self.nanosPerMicro
    }

    private static let paragraphs = 2_000
    private static let keystrokes = 400
    private static let percentile = 95
    private static let nanosPerMicro = 1_000.0
    /// Generous for simulators under CI load; devices are far quicker.
    private static let budget = 4_000.0
}
