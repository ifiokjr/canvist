@testable import Canvist
import UIKit
import XCTest

/// The view in a window, driven through `UITextInput` the way UIKit's text
/// system drives it: keyboards, input methods, autocorrect, and selection.
@MainActor
final class CanvistViewTests: XCTestCase {
    private var window: UIWindow!
    private let kit = StarterKit()

    override func setUp() async throws {
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
        window.makeKeyAndVisible()
    }

    override func tearDown() async throws {
        window.isHidden = true
        window = nil
    }

    private func view(_ markdown: String = "") throws -> CanvistView {
        let view = try CanvistView(extensions: kit.extensions)
        try view.editor.load(format: .markdown, content: markdown)
        view.frame = window.bounds
        window.addSubview(view)
        view.layoutIfNeeded()

        return view
    }

    private func range(_ start: Int, _ end: Int) -> CanvistTextRange {
        CanvistTextRange(start..<end)
    }

    func testPaintsTheDocumentAndSizesItsContent() throws {
        let view = try view("# Title\n\nBody text")

        XCTAssertGreaterThan(view.contentSize.height, view.textInsets.top + view.textInsets.bottom)
        let texts = view.items.compactMap { item -> String? in
            guard case .text(let text, _, _, _, _, _, _, _, _) = item else { return nil }
            return text
        }
        XCTAssertEqual(texts.joined(), "TitleBody text")
    }

    func testTypingAndBackspace() throws {
        let view = try view()

        view.insertText("Hi")
        view.insertText(" there")
        view.deleteBackward()

        XCTAssertEqual(view.editor.plainText(), "Hi ther")
        XCTAssertEqual(view.offsets(view.selectedTextRange!), OffsetRange(start: 7, end: 7))
        XCTAssertTrue(view.hasText)
    }

    func testMarkedTextFromAnInputMethod() throws {
        let view = try view()

        view.setMarkedText("k", selectedRange: NSRange(location: 1, length: 0))
        view.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0))
        view.setMarkedText("漢字", selectedRange: NSRange(location: 0, length: 2))
        XCTAssertEqual(view.offsets(view.markedTextRange!), OffsetRange(start: 0, end: 2))
        let converting = view.offsets(view.selectedTextRange!)
        XCTAssertEqual(converting, OffsetRange(start: 0, end: 2), "the clause being converted")

        view.unmarkText()
        XCTAssertNil(view.markedTextRange)
        XCTAssertEqual(view.editor.plainText(), "漢字")
        XCTAssertTrue(view.undoManager!.canUndo)

        view.undoManager!.undo()
        XCTAssertEqual(view.editor.plainText(), "", "the whole composition undoes in one step")
    }

    func testAutocorrectReplacesARange() throws {
        let view = try view("teh cat")

        view.replace(range(0, 3), withText: "the")

        XCTAssertEqual(view.text(in: range(0, 7)), "the cat")
    }

    func testGeometryForCaretsSelectionsAndHitTesting() throws {
        let view = try view("hello world")
        view.selectedTextRange = range(5, 5)

        let start = view.caretRect(for: view.beginningOfDocument)
        let caret = view.caretRect(for: CanvistTextPosition(5))
        XCTAssertGreaterThan(caret.minX, start.minX)
        XCTAssertEqual(start.minX, view.textInsets.left, accuracy: 2)

        let closest = view.closestPosition(to: CGPoint(x: caret.midX, y: caret.midY))
        XCTAssertEqual((closest as? CanvistTextPosition)?.offset, 5)

        let rects = view.selectionRects(for: range(0, 11))
        XCTAssertEqual(rects.count, 1)
        XCTAssertTrue(rects[0].containsStart && rects[0].containsEnd)
        let first = view.firstRect(for: range(6, 11))
        XCTAssertEqual(first.minX, view.caretRect(for: CanvistTextPosition(6)).midX, accuracy: 2)
    }

    func testMovingByLinesAndCharacters() throws {
        let view = try view(String(repeating: "word ", count: 60))
        let start = view.beginningOfDocument

        let below = view.position(from: start, in: .down, offset: 1) as? CanvistTextPosition
        XCTAssertNotNil(below)
        XCTAssertGreaterThan(below!.offset, 0)
        XCTAssertNil(view.position(from: start, in: .up, offset: 1), "the first line")
        XCTAssertEqual((view.position(from: start, in: .right, offset: 3) as? CanvistTextPosition)?.offset, 3)
        XCTAssertNil(view.position(from: start, in: .left, offset: 1))
    }

    func testCopyAndPasteThroughThePasteboard() throws {
        let view = try view("Hello **bold**")
        let pasteboard = UIPasteboard.withUniqueName()
        view.pasteboard = pasteboard
        view.selectedTextRange = range(6, 10)

        view.copy(nil)
        XCTAssertEqual(pasteboard.string, "bold")

        view.selectedTextRange = range(0, 0)
        view.paste(nil)
        XCTAssertEqual(view.editor.export(format: .markdown), "**bold**Hello **bold**")
    }

    func testFormattingFromTheEditMenu() throws {
        let view = try view("make this bold")
        view.selectedTextRange = range(10, 14)

        let bold = #selector(UIResponderStandardEditActions.toggleBoldface(_:))
        XCTAssertTrue(view.canPerformAction(bold, withSender: nil))
        view.toggleBoldface(nil)

        XCTAssertEqual(view.editor.export(format: .markdown), "make this **bold**")
        let menu = view.editMenu(for: range(10, 14), suggestedActions: [])
        XCTAssertTrue(menu?.children.contains { ($0 as? UIMenu)?.title == "Format" } ?? false)
    }

    func testCommandsFromOutsideTellTheInputSystem() throws {
        let view = try view("text")
        let delegate = RecordingInputDelegate()
        view.inputDelegate = delegate

        _ = try view.editor.run(command: kit.heading.toggleHeading(level: 1))

        XCTAssertEqual(delegate.calls, ["textWillChange", "textDidChange", "selectionWillChange", "selectionDidChange"])
    }

    func testInputFromTheTextSystemIsNotEchoedBack() throws {
        let view = try view()
        let delegate = RecordingInputDelegate()
        view.inputDelegate = delegate

        view.insertText("a")

        XCTAssertEqual(delegate.calls, [])
    }

    func testTappingACheckboxTogglesTheTask() throws {
        let view = try view("- [ ] task")
        guard let box = view.items.lazy.compactMap({ item -> Bounds? in
            guard case .checkbox(let bounds, _) = item else { return nil }
            return bounds
        }).first else { return XCTFail("no checkbox") }

        let point = view.viewRect(box).center
        XCTAssertNotNil(view.checkbox(at: point))
        XCTAssertTrue(try view.editor.toggleTaskAt(x: box.x + 1, y: box.y + 1))
        XCTAssertEqual(view.editor.export(format: .markdown), "- [x] task")
    }

    func testReadsLinesToVoiceOver() throws {
        let view = try view("# Heading\n\nFirst paragraph")

        XCTAssertEqual(view.accessibilityValue, "Heading\nFirst paragraph")
        XCTAssertEqual(view.accessibilityContent(forLineNumber: 0), "Heading")
        XCTAssertEqual(view.accessibilityContent(forLineNumber: 1), "First paragraph")
        XCTAssertNil(view.accessibilityContent(forLineNumber: 2))
    }

    func testFollowsDarkMode() throws {
        let view = try view("text")
        let light = view.backgroundColor

        view.overrideUserInterfaceStyle = .dark
        view.layoutIfNeeded()

        XCTAssertNotEqual(view.backgroundColor, light)
    }

    /// Keystroke to paint: a character through `UITextInput`, the relayout,
    /// the display list, and Core Graphics drawing the damaged area.
    func testKeystrokeToPaintStaysWithinBudget() throws {
        let paragraph = "The quick brown fox jumps over the lazy dog, again and again."
        let view = try view(Array(repeating: paragraph, count: Self.paragraphs).joined(separator: "\n\n"))
        view.selectedTextRange = range(10, 10)
        paint(view)

        let samples = (0..<Self.keystrokes)
            .map { index in keystroke(view, typing: index % 2 == 0) }
            .sorted()
        let median = samples[samples.count / 2]
        let p95 = samples[samples.count * Self.percentile / 100]

        print("PERF swift keystroke-to-paint median=\(Int(median))µs p95=\(Int(p95))µs")
        XCTAssertLessThan(p95, Self.budget * perfSlack, "keystroke-to-paint p95 \(p95)µs is over its budget")
    }

    /// Type or delete a character and paint it; returns microseconds.
    private func keystroke(_ view: CanvistView, typing: Bool) -> Double {
        let start = DispatchTime.now().uptimeNanoseconds

        if typing {
            view.insertText("x")
        } else {
            view.deleteBackward()
        }

        paint(view)

        return Double(DispatchTime.now().uptimeNanoseconds - start) / Self.nanosPerMicro
    }

    private func paint(_ view: CanvistView) {
        view.layoutIfNeeded()
        view.canvas.layer.displayIfNeeded()
    }

    private static let paragraphs = 2_000
    private static let keystrokes = 300
    private static let percentile = 95
    private static let nanosPerMicro = 1_000.0
    /// One frame at 60 Hz, with room for simulators under CI load.
    private static let budget = 16_000.0
}

/// Records what the view tells the text input system.
private final class RecordingInputDelegate: NSObject, UITextInputDelegate {
    var calls: [String] = []

    func selectionWillChange(_ textInput: UITextInput?) { calls.append("selectionWillChange") }
    func selectionDidChange(_ textInput: UITextInput?) { calls.append("selectionDidChange") }
    func textWillChange(_ textInput: UITextInput?) { calls.append("textWillChange") }
    func textDidChange(_ textInput: UITextInput?) { calls.append("textDidChange") }
    @available(iOS 18.4, *)
    func conversationContext(_ context: UIConversationContext?, didChange textInput: UITextInput?) {}
}

private extension CGRect {
    var center: CGPoint { CGPoint(x: midX, y: midY) }
}
