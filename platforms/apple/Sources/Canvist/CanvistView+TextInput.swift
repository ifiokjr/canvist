import UIKit

/// `UITextInput`: what the keyboard, input methods, autocorrect, dictation,
/// and `UITextInteraction` ask of a text view. Positions are UTF-16 offsets
/// into the editor's text (``CanvistTextPosition``); rectangles are in this
/// view's coordinates.
extension CanvistView: UITextInput {
    override open var canBecomeFirstResponder: Bool { true }

    @discardableResult
    override open func becomeFirstResponder() -> Bool {
        let became = super.becomeFirstResponder()

        if became {
            editor.setFocused(focused: true)
            revealCaret = true
            setNeedsLayout()
        }

        return became
    }

    @discardableResult
    override open func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()

        if resigned {
            _ = fromInput("unmark") { try editor.unmarkText() }
            editor.setFocused(focused: false)
        }

        return resigned
    }

    // MARK: UIKeyInput
    public var hasText: Bool { editor.textLength() > 0 }

    public func insertText(_ text: String) {
        _ = fromInput("insertText") { try editor.insertText(text: text) }
    }

    public func deleteBackward() {
        _ = fromInput("deleteBackward") { try editor.deleteBackward() }
    }

    // MARK: Text and ranges
    public func text(in range: UITextRange) -> String? {
        editor.textIn(range: offsets(range))
    }

    public func replace(_ range: UITextRange, withText text: String) {
        _ = fromInput("replace") { try editor.replace(range: offsets(range), text: text) }
    }

    public var selectedTextRange: UITextRange? {
        get { CanvistTextRange(editor.selection().range) }
        set {
            guard let newValue else { return }

            let range = offsets(newValue)
            _ = fromInput("select") { try editor.select(anchor: range.start, head: range.end) }
        }
    }

    public var markedTextRange: UITextRange? {
        editor.markedRange().map(CanvistTextRange.init)
    }

    public var markedTextStyle: [NSAttributedString.Key: Any]? {
        get { nil }
        // The editor underlines marked text itself.
        set {}
    }

    public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
        let selected = OffsetRange(
            start: UInt32(max(selectedRange.location, 0)),
            end: UInt32(max(selectedRange.location + selectedRange.length, 0))
        )
        _ = fromInput("setMarkedText") { try editor.setMarkedText(text: markedText ?? "", selected: selected) }
    }

    public func unmarkText() {
        _ = fromInput("unmarkText") { try editor.unmarkText() }
    }

    public var beginningOfDocument: UITextPosition { CanvistTextPosition(0) }
    public var endOfDocument: UITextPosition { CanvistTextPosition(Int(editor.textLength())) }

    public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        let (from, to) = (offset(fromPosition), offset(toPosition))

        return CanvistTextRange(min(from, to)..<max(from, to))
    }

    public func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
        let moved = self.offset(position) + offset

        guard (0...Int(editor.textLength())).contains(moved) else { return nil }

        return CanvistTextPosition(moved)
    }

    public func position(
        from position: UITextPosition,
        in direction: UITextLayoutDirection,
        offset: Int
    ) -> UITextPosition? {
        switch direction {
        case .left: return self.position(from: position, offset: -offset)
        case .right: return self.position(from: position, offset: offset)
        case .up: return verticalPosition(from: position, lines: offset, up: true)
        case .down: return verticalPosition(from: position, lines: offset, up: false)
        @unknown default: return nil
        }
    }

    private func verticalPosition(from position: UITextPosition, lines: Int, up: Bool) -> UITextPosition? {
        var current = UInt32(offset(position))

        for _ in 0..<max(lines, 0) {
            guard let next = editor.verticalOffset(offset: current, up: up, goalX: nil) else { return nil }
            current = next
        }

        return CanvistTextPosition(Int(current))
    }

    public func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        let (a, b) = (offset(position), offset(other))

        return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
    }

    public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
        offset(toPosition) - offset(from)
    }

    public var inputDelegate: UITextInputDelegate? {
        get { textInputDelegate }
        set { textInputDelegate = newValue }
    }

    public var tokenizer: UITextInputTokenizer {
        UITextInputStringTokenizer(textInput: self)
    }

    public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        switch direction {
        case .left, .up: return range.start
        default: return range.end
        }
    }

    public func characterRange(
        byExtending position: UITextPosition,
        in direction: UITextLayoutDirection
    ) -> UITextRange? {
        let start = offset(position)

        switch direction {
        case .left, .up: return CanvistTextRange(max(start - 1, 0)..<start)
        default: return CanvistTextRange(start..<min(start + 1, Int(editor.textLength())))
        }
    }

    public func baseWritingDirection(
        for position: UITextPosition,
        in direction: UITextStorageDirection
    ) -> NSWritingDirection {
        .natural
    }

    public func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {
        // Each paragraph's direction comes from its text (UAX #9).
    }

    // MARK: Geometry
    public func firstRect(for range: UITextRange) -> CGRect {
        editor.firstRect(range: offsets(range)).map(viewRect) ?? .null
    }

    public func caretRect(for position: UITextPosition) -> CGRect {
        editor.caretRect(offset: UInt32(offset(position))).map(viewRect) ?? .zero
    }

    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        let rects = editor.selectionRects(range: offsets(range))

        return rects.enumerated().map { index, rect in
            CanvistSelectionRect(viewRect(rect), first: index == 0, last: index == rects.count - 1)
        }
    }

    public func closestPosition(to point: CGPoint) -> UITextPosition? {
        let point = documentPoint(point)

        return editor.offsetAt(x: point.x, y: point.y).map { CanvistTextPosition(Int($0)) }
    }

    public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
        guard let closest = closestPosition(to: point) else { return nil }

        let offsets = (range as? CanvistTextRange)?.offsets ?? 0..<0

        return CanvistTextPosition(min(max(offset(closest), offsets.lowerBound), offsets.upperBound))
    }

    public func characterRange(at point: CGPoint) -> UITextRange? {
        guard let position = closestPosition(to: point) else { return nil }

        let start = offset(position)
        let text = editor.textIn(range: OffsetRange(start: UInt32(start), end: UInt32(start + 2)))
        let length = text.first.map { String($0).utf16.count } ?? 0

        return CanvistTextRange(start..<start + length)
    }

    // MARK: Conversions
    func offset(_ position: UITextPosition) -> Int {
        (position as? CanvistTextPosition)?.offset ?? 0
    }

    func offsets(_ range: UITextRange) -> OffsetRange {
        (range as? CanvistTextRange)?.range ?? OffsetRange(start: 0, end: 0)
    }
}
