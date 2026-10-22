import UIKit

/// VoiceOver: the view is one text element whose value is the document,
/// read line by line through `UIAccessibilityReadingContent`, with a rotor
/// that moves between headings.
extension CanvistView: UIAccessibilityReadingContent {
  func setUpAccessibility() {
    isAccessibilityElement = true
    accessibilityLabel = String(
      localized: "Editor", comment: "The accessibility label of the canvist editor")
    accessibilityTextualContext = .wordProcessing
    accessibilityCustomRotors = [headingRotor()]

    // Lets VoiceOver (and UI tests) edit the text as they would a text
    // view's: move by character, word, and line, and type.
    if #available(iOS 18.1, *) {
      accessibilityTextInputResponder = self
    }
  }

  /// The document's text, read when asked so it is never stale.
  override open var accessibilityValue: String? {
    get { editor.plainText() }
    set {}
  }

  /// The lines on screen, top to bottom.
  private var visibleLines: [LineInfo] {
    let top = Float(bounds.minY - textInsets.top)

    return editor.lines(top: top, bottom: top + Float(bounds.height))
  }

  public func accessibilityLineNumber(for point: CGPoint) -> Int {
    let (_, y) = documentPoint(point)

    return visibleLines.firstIndex { line in y < line.bounds.y + line.bounds.height } ?? NSNotFound
  }

  public func accessibilityContent(forLineNumber lineNumber: Int) -> String? {
    let lines = visibleLines

    guard lines.indices.contains(lineNumber) else { return nil }

    return editor.textIn(range: lines[lineNumber].range)
  }

  public func accessibilityFrame(forLineNumber lineNumber: Int) -> CGRect {
    let lines = visibleLines

    guard lines.indices.contains(lineNumber) else { return .zero }

    return UIAccessibility.convertToScreenCoordinates(viewRect(lines[lineNumber].bounds), in: self)
  }

  public func accessibilityPageContent() -> String? {
    visibleLines.map { editor.textIn(range: $0.range) }.joined(separator: "\n")
  }

  /// A rotor that selects the next or previous heading's text.
  private func headingRotor() -> UIAccessibilityCustomRotor {
    UIAccessibilityCustomRotor(systemType: .heading) { [weak self] predicate in
      guard let self else { return nil }

      return MainActor.assumeIsolated { self.heading(after: predicate) }
    }
  }

  private func heading(
    after predicate: UIAccessibilityCustomRotorSearchPredicate
  ) -> UIAccessibilityCustomRotorItemResult? {
    let headings = editor.blocks(top: 0, bottom: editor.contentHeight()).filter { block in
      if case .heading = block.role { return true }
      return false
    }
    let current =
      (predicate.currentItem.targetRange as? CanvistTextRange)?.offsets.lowerBound
      ?? Int(editor.selection().head)
    let next =
      predicate.searchDirection == .next
      ? headings.first { Int($0.range.start) > current }
      : headings.last { Int($0.range.start) < current }

    return next.map { heading in
      UIAccessibilityCustomRotorItemResult(
        targetElement: self, targetRange: CanvistTextRange(heading.range))
    }
  }
}
