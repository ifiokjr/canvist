import UIKit

/// A position in a ``CanvistView``'s text: a UTF-16 offset into the
/// document's text blocks joined by line feeds.
public final class CanvistTextPosition: UITextPosition {
    /// The UTF-16 offset.
    public let offset: Int

    init(_ offset: Int) {
        self.offset = offset
    }

    override public var description: String {
        "CanvistTextPosition(\(offset))"
    }
}

/// A range of a ``CanvistView``'s text.
public final class CanvistTextRange: UITextRange {
    /// The UTF-16 offsets, start first.
    public let offsets: Range<Int>

    init(_ offsets: Range<Int>) {
        self.offsets = offsets
    }

    convenience init(_ range: OffsetRange) {
        self.init(Int(range.start)..<Int(range.end))
    }

    override public var start: UITextPosition { CanvistTextPosition(offsets.lowerBound) }
    override public var end: UITextPosition { CanvistTextPosition(offsets.upperBound) }
    override public var isEmpty: Bool { offsets.isEmpty }

    var range: OffsetRange {
        OffsetRange(start: UInt32(offsets.lowerBound), end: UInt32(offsets.upperBound))
    }

    override public var description: String {
        "CanvistTextRange(\(offsets))"
    }
}

/// One rectangle of a selection, for the system's highlight and handles.
final class CanvistSelectionRect: UITextSelectionRect {
    private let frame: CGRect
    private let first: Bool
    private let last: Bool

    init(_ frame: CGRect, first: Bool, last: Bool) {
        self.frame = frame
        self.first = first
        self.last = last
    }

    override var rect: CGRect { frame }
    override var writingDirection: NSWritingDirection { .natural }
    override var containsStart: Bool { first }
    override var containsEnd: Bool { last }
    override var isVertical: Bool { false }
}

extension TextSelection {
    /// The selected range, start first.
    public var range: OffsetRange {
        OffsetRange(start: min(anchor, head), end: max(anchor, head))
    }

    /// Whether nothing is selected: a caret.
    public var isCollapsed: Bool { anchor == head }
}
