import XCTest

final class CanvistExampleUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testAppendingTextRoundTripsThroughRust() {
        let app = XCUIApplication()
        app.launch()

        let document = app.staticTexts["document"]
        XCTAssertEqual(document.label, "Empty")

        let draft = app.textFields["draft"]
        draft.tap()
        draft.typeText("hello")
        app.buttons["append"].tap()
        XCTAssertEqual(document.label, "hello")

        draft.tap()
        draft.typeText(" world")
        app.buttons["append"].tap()
        XCTAssertEqual(document.label, "hello world")

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "canvist-example"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
