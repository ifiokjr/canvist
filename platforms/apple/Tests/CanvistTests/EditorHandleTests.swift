import Canvist
import XCTest

final class EditorHandleTests: XCTestCase {
    func testStartsEmpty() {
        XCTAssertEqual(EditorHandle().text(), "")
    }

    func testInsertsTextThroughRust() throws {
        let editor = EditorHandle()
        try editor.insertText(offset: 0, text: "world")
        try editor.insertText(offset: 0, text: "hello ")
        XCTAssertEqual(editor.text(), "hello world")
    }

    func testOffsetsCountUnicodeScalarValues() throws {
        let editor = EditorHandle()
        try editor.insertText(offset: 0, text: "héllo")
        try editor.insertText(offset: 2, text: "—")
        XCTAssertEqual(editor.text(), "hé—llo")
    }

    func testRejectsOffsetsPastTheEnd() {
        let editor = EditorHandle()
        XCTAssertThrowsError(try editor.insertText(offset: 1, text: "x")) { error in
            XCTAssertEqual(error as? EditorError, .OffsetOutOfBounds(offset: 1, length: 0))
        }
        XCTAssertEqual(editor.text(), "")
    }

    func testReportsTheRustCoreVersion() {
        let version = EditorHandle().version()
        XCTAssertNotNil(version.wholeMatch(of: /\d+\.\d+\.\d+.*/), "unexpected version \(version)")
    }
}
