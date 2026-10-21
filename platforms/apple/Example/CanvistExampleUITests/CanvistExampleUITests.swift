import XCTest

/// The example app driven like a person would: the software keyboard, the
/// toolbar, double taps, the edit menu, and two editors collaborating.
final class CanvistExampleUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["-emptyDocument"]
        app.launch()
    }

    private func editor(_ name: String = "Editor") -> XCUIElement {
        app.descendants(matching: .any)["\(name.lowercased())-editor"].firstMatch
    }

    private func value(of element: XCUIElement) -> String {
        element.value as? String ?? ""
    }

    private func isSelected(_ button: String) -> Bool {
        app.buttons[button].isSelected
    }

    @MainActor
    func testTypingFormattingAndUndo() {
        let editor = editor()
        editor.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: Self.timeout), "the keyboard appears")
        app.typeText("Hello")
        XCTAssertEqual(value(of: editor), "Hello")

        app.buttons["bold"].tap()
        app.typeText(" bold")
        XCTAssertEqual(value(of: editor), "Hello bold")
        XCTAssertTrue(isSelected("bold"), "the typed text is bold")

        app.buttons["undo"].tap()
        XCTAssertFalse(value(of: editor).contains("bold"), value(of: editor))

        attach("formatting")
    }

    @MainActor
    func testInputRulesMakeHeadingsAndLists() {
        let editor = editor()
        editor.tap()
        app.typeText("## Title")
        XCTAssertTrue(isSelected("heading"))

        app.typeText("\n- item")
        XCTAssertTrue(isSelected("bulletList"))
        XCTAssertEqual(value(of: editor), "Title\nitem\n")
    }

    @MainActor
    func testDoubleTapSelectsAWordAndShowsTheEditMenu() {
        let editor = editor()
        editor.tap()
        app.typeText("select these words")

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
            .withOffset(Self.firstWord)
            .doubleTap()
        let copy = app.menuItems["Copy"].exists ? app.menuItems["Copy"] : app.buttons["Copy"]
        XCTAssertTrue(copy.waitForExistence(timeout: Self.timeout), "the edit menu offers Copy")
        attach("edit-menu")

        app.buttons["italic"].tap()
        XCTAssertTrue(isSelected("italic"), "the selected word turned italic")
    }

    @MainActor
    func testTwoEditorsCollaborate() {
        app.tabBars.buttons["Collaborate"].tap()
        let alice = editor("Alice")
        let bob = editor("Bob")
        XCTAssertTrue(value(of: bob).contains("Shared notes"), value(of: bob))

        alice.tap()
        app.typeText(" Hi from Alice")
        let arrived = NSPredicate(format: "value CONTAINS %@", "Hi from Alice")
        expectation(for: arrived, evaluatedWith: bob)
        waitForExpectations(timeout: Self.timeout)
        attach("collaboration")
    }

    /// How long to wait for the keyboard, the edit menu, and collaborators.
    private static let timeout: TimeInterval = 5
    /// A point inside the first word of the first line, from the editor's
    /// top-left corner.
    private static let firstWord = CGVector(dx: 40, dy: 28)

    private func attach(_ name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
