import UIKit
import UniformTypeIdentifiers

/// Hardware keys, the edit menu, the pasteboard, undo, task checkboxes, and
/// the keyboard.
extension CanvistView {
  // MARK: Hardware keys
  override open func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
    let unhandled = presses.filter { press in !handle(press) }

    if !unhandled.isEmpty {
      super.pressesBegan(unhandled, with: event)
    }
  }

  /// Run the editor's binding for `press`, such as Command-B, Tab in a
  /// list, or Option-Left. Typed characters are left to the text input
  /// system, which feeds input methods, and so is everything while an
  /// input method is composing.
  private func handle(_ press: UIPress) -> Bool {
    guard let key = press.key, editor.markedRange() == nil, let name = KeyName(key) else {
      return false
    }

    let modifiers = Modifiers(
      shift: key.modifierFlags.contains(.shift),
      ctrl: key.modifierFlags.contains(.control),
      alt: key.modifierFlags.contains(.alternate),
      meta: key.modifierFlags.contains(.command)
    )
    let handled =
      fromInput("press") { try editor.press(key: name.dom, modifiers: modifiers) } ?? false

    if handled {
      textInputDelegate?.selectionWillChange(self)
      textInputDelegate?.selectionDidChange(self)
    }

    return handled
  }

  // MARK: Edit menu
  override open func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
    let selected = !editor.selection().isCollapsed

    switch action {
    case #selector(copy(_:)), #selector(cut(_:)):
      return selected
    case #selector(paste(_:)):
      return pasteboard.hasStrings || pasteboard.contains(pasteboardTypes: [UTType.html.identifier])
    case #selector(selectAll(_:)):
      return hasText
    default:
      guard let format = Self.formatActions[action] else {
        return super.canPerformAction(action, withSender: sender)
      }

      return selected && extensions.names.contains(format)
    }
  }

  /// The edit menu's formatting actions, and the extension each needs.
  private static let formatActions: [Selector: String] = [
    #selector(toggleBoldface(_:)): Extensions.bold.name,
    #selector(toggleItalics(_:)): Extensions.italic.name,
    #selector(toggleUnderline(_:)): Extensions.underline.name,
  ]

  override open func copy(_ sender: Any?) {
    guard let content = fromInput("copy", { try editor.copy() }) ?? nil else { return }

    write(content)
  }

  override open func cut(_ sender: Any?) {
    guard let content = fromInput("cut", { try editor.cut() }) ?? nil else { return }

    write(content)
    notifyExternalChange()
  }

  override open func paste(_ sender: Any?) {
    if let data = pasteboard.data(forPasteboardType: UTType.html.identifier),
      let html = String(data: data, encoding: .utf8)
    {
      _ = fromInput("paste") { try editor.pasteHtml(html: html) }
    } else if let text = pasteboard.string {
      _ = fromInput("paste") { try editor.pasteText(text: text) }
    }

    notifyExternalChange()
  }

  override open func selectAll(_ sender: Any?) {
    _ = fromInput("selectAll") { try editor.selectAll() }
    notifyExternalChange()
  }

  override open func toggleBoldface(_ sender: Any?) {
    run(Extensions.bold.handle.toggleBold())
  }

  override open func toggleItalics(_ sender: Any?) {
    run(Extensions.italic.handle.toggleItalic())
  }

  override open func toggleUnderline(_ sender: Any?) {
    run(Extensions.underline.handle.toggleUnderline())
  }

  /// The edit menu, with a Format menu of the registered extensions'
  /// block and mark commands after the system's actions.
  public func editMenu(for textRange: UITextRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
    let format = formatActions()

    guard !format.isEmpty else { return UIMenu(children: suggestedActions) }

    let menu = UIMenu(title: "Format", image: UIImage(systemName: "textformat"), children: format)

    return UIMenu(children: suggestedActions + [menu])
  }

  private func formatActions() -> [UIAction] {
    let names = Set(extensions.names)
    let code = Extensions.code.handle.toggleCode()
    let heading = Extensions.heading.handle.toggleHeading(level: 2)
    let list = Extensions.bulletList.handle.toggleBulletList()
    let actions: [(String, String, String, Command)] = [
      (
        Extensions.strike.name, "Strikethrough", "strikethrough",
        Extensions.strike.handle.toggleStrike()
      ),
      (Extensions.code.name, "Code", "chevron.left.forwardslash.chevron.right", code),
      (Extensions.heading.name, "Heading", "textformat.size", heading),
      (
        Extensions.blockquote.name, "Quote", "text.quote",
        Extensions.blockquote.handle.toggleBlockquote()
      ),
      (Extensions.bulletList.name, "Bulleted List", "list.bullet", list),
      (
        Extensions.taskList.name, "Checklist", "checklist",
        Extensions.taskList.handle.toggleTaskList()
      ),
    ]

    return
      actions
      .filter { names.contains($0.0) }
      .map { _, title, symbol, command in
        UIAction(title: title, image: UIImage(systemName: symbol)) { [weak self] _ in
          self?.run(command)
        }
      }
  }

  /// Run `command` as the user's own edit.
  func run(_ command: Command) {
    _ = fromInput("run") { try editor.run(command: command) }
    notifyExternalChange()
  }

  /// Tell the text input system about a change it did not ask for, made
  /// while handling one of its calls.
  func notifyExternalChange() {
    textInputDelegate?.textWillChange(self)
    textInputDelegate?.textDidChange(self)
    textInputDelegate?.selectionWillChange(self)
    textInputDelegate?.selectionDidChange(self)
  }

  private func write(_ content: ClipboardContent) {
    pasteboard.items = [
      [
        UTType.html.identifier: content.html,
        UTType.utf8PlainText.identifier: content.text,
      ]
    ]
  }

  // MARK: Undo
  override open var undoManager: UndoManager? { undoBridge }

  func performUndo(redo: Bool) {
    _ = fromInput("undo") { redo ? try editor.redo() : try editor.undo() }
    notifyExternalChange()
  }

  // MARK: Task checkboxes
  func addCheckboxGesture() {
    checkboxTap.addTarget(self, action: #selector(toggleTask(_:)))
    addGestureRecognizer(checkboxTap)
  }

  /// Add the text interaction, which places the caret on tap, selects,
  /// and shows the edit menu, behind taps on checkboxes.
  func installTextInteraction() {
    addInteraction(interaction)

    // Taps on text place the caret only once this has failed, which it
    // does at once anywhere but a checkbox.
    for gesture in interaction.gesturesForFailureRequirements {
      gesture.require(toFail: checkboxTap)
    }
  }

  override open func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
    if gesture === checkboxTap {
      return checkbox(at: gesture.location(in: self)) != nil
    }

    if gesture === focusTap {
      return !isFirstResponder
    }

    return super.gestureRecognizerShouldBegin(gesture)
  }

  /// Focus on a tap anywhere, as text views do, with the caret where the
  /// tap landed; the text interaction handles taps once focused.
  func addFocusGesture() {
    focusTap.addTarget(self, action: #selector(focus(_:)))
    focusTap.require(toFail: checkboxTap)
    addGestureRecognizer(focusTap)
  }

  @objc private func focus(_ gesture: UITapGestureRecognizer) {
    let point = documentPoint(gesture.location(in: self))

    if let offset = editor.offsetAt(x: point.x, y: point.y) {
      _ = fromInput("focus") { try editor.select(anchor: offset, head: offset) }
    }

    becomeFirstResponder()
    notifyExternalChange()
  }

  @objc private func toggleTask(_ gesture: UITapGestureRecognizer) {
    let point = documentPoint(gesture.location(in: self))
    _ = fromInput("toggleTask") { try editor.toggleTaskAt(x: point.x, y: point.y) }
    notifyExternalChange()
  }

  /// The checkbox under `point`, in this view's coordinates.
  func checkbox(at point: CGPoint) -> Bounds? {
    let (x, y) = documentPoint(point)

    for case .checkbox(let bounds, _) in items {
      let hit = DisplayPainter.rect(bounds).insetBy(dx: -Self.checkboxSlop, dy: -Self.checkboxSlop)

      if hit.contains(CGPoint(x: CGFloat(x), y: CGFloat(y))) {
        return bounds
      }
    }

    return nil
  }

  private static let checkboxSlop: CGFloat = 8

  // MARK: Keyboard
  func observeKeyboard() {
    NotificationCenter.default.addObserver(
      self,
      selector: #selector(keyboardWillChangeFrame(_:)),
      name: UIResponder.keyboardWillChangeFrameNotification,
      object: nil
    )
  }

  /// Keep the text above the keyboard.
  @objc private func keyboardWillChangeFrame(_ notification: Notification) {
    guard let window,
      let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect
    else { return }

    let overlap = max(convert(bounds, to: window).maxY - frame.minY, 0)
    let inset = max(overlap - safeAreaInsets.bottom, 0)
    contentInset.bottom = inset
    verticalScrollIndicatorInsets.bottom = inset
    revealCaret = true
    setNeedsLayout()
  }
}

/// An undo manager that undoes in the editor, for the system's undo gestures
/// (shake, three-finger swipe) and the hardware keyboard's Command-Z.
final class EditorUndoManager: UndoManager {
  private weak var view: CanvistView?

  @MainActor
  init(view: CanvistView) {
    self.view = view
    super.init()
  }

  override var canUndo: Bool {
    MainActor.assumeIsolated { view?.editor.canUndo() ?? false }
  }

  override var canRedo: Bool {
    MainActor.assumeIsolated { view?.editor.canRedo() ?? false }
  }

  override func undo() {
    MainActor.assumeIsolated { view?.performUndo(redo: false) }
  }

  override func redo() {
    MainActor.assumeIsolated { view?.performUndo(redo: true) }
  }
}

/// A key as the editor's key bindings name it, after DOM `KeyboardEvent.key`.
@MainActor
struct KeyName {
  let dom: String

  /// The binding name of `key`, or `nil` for keys the text input system
  /// handles: typed characters without Command or Control, and Return.
  init?(_ key: UIKey) {
    if let special = Self.special[key.keyCode] {
      dom = special

      return
    }

    let shortcut = key.modifierFlags.contains(.command) || key.modifierFlags.contains(.control)
    let character = key.charactersIgnoringModifiers.lowercased()

    guard shortcut, character.count == 1 else { return nil }

    dom = character
  }

  private static let special: [UIKeyboardHIDUsage: String] = [
    .keyboardLeftArrow: "ArrowLeft",
    .keyboardRightArrow: "ArrowRight",
    .keyboardUpArrow: "ArrowUp",
    .keyboardDownArrow: "ArrowDown",
    .keyboardTab: "Tab",
    .keyboardHome: "Home",
    .keyboardEnd: "End",
    .keyboardPageUp: "PageUp",
    .keyboardPageDown: "PageDown",
    .keyboardDeleteForward: "Delete",
    .keyboardEscape: "Escape",
  ]
}
