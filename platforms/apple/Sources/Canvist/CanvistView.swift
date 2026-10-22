import os
import UIKit

/// A rich-text editor view: canvist's Rust engine, painted with Core
/// Graphics and edited through UIKit's text input system.
///
/// Typing, the software and hardware keyboards, input methods (marked text),
/// autocorrect, and dictation arrive through `UITextInput`; the system's
/// `UITextInteraction` draws the caret, the selection and its handles, the
/// loupe, and the edit menu. Text is measured with CoreText
/// (``CoreTextMeasurer``) and painted in the same fonts. The view scrolls
/// like a text view and lays out only what is on screen.
///
/// Run commands on ``editor`` with the typed handles; the view repaints
/// itself:
///
/// ```swift
/// let kit = StarterKit()
/// let view = try CanvistView(extensions: kit.extensions)
/// try view.editor.load(format: .markdown, content: "# Hello")
/// _ = try view.editor.run(command: kit.bold.toggleBold())
/// ```
///
/// The view is a plain `UIScrollView` subclass with no view controller, so
/// SwiftUI (``CanvistEditor``), React Native, and Lynx can embed it.
@MainActor
open class CanvistView: UIScrollView {
    /// The editor this view shows and edits.
    public let editor: Editor
    /// The extensions the editor registered.
    public let extensions: ExtensionSet
    /// Called on the main thread after every change, for example to update a
    /// toolbar or send collaboration updates.
    public var onChange: ((EditorChange) -> Void)?
    /// Called on the main thread when the view gains (`true`) or loses focus.
    public var onFocusChange: ((Bool) -> Void)?

    /// Space between the view's edges and the text.
    public var textInsets = CanvistView.defaultTextInsets {
        didSet { setNeedsLayout() }
    }

    /// The pasteboard copy, cut, and paste use.
    public var pasteboard = UIPasteboard.general

    /// The body text size before Dynamic Type scales it.
    public var baseFontSize = CanvistView.defaultFontSize {
        didSet { applyTheme() }
    }

    let measurer: CoreTextMeasurer
    let canvas: CanvasView
    let interaction = UITextInteraction(for: .editable)
    let checkboxTap = UITapGestureRecognizer()
    let focusTap = UITapGestureRecognizer()
    lazy var undoBridge = EditorUndoManager(view: self)
    weak var textInputDelegate: UITextInputDelegate?
    /// Calls from the text input system in progress, whose changes it
    /// already knows about.
    var inputCalls = 0
    /// Whether the caret should scroll into view after the next frame.
    var revealCaret = false
    /// What is painted now.
    var items: [DisplayItem] = []
    let logger = Logger(subsystem: "dev.canvist", category: "CanvistView")

    private let relay: ChangeRelay
    private let observer: UInt64
    private var theme: ThemeOptions?

    /// Options for an editor this view drives: `extensions`, Apple key
    /// bindings (`Mod` is Command), and the caret and selection left to
    /// UIKit.
    public static func editorOptions(extensions: ExtensionSet) -> EditorOptions {
        var options = defaultEditorOptions()
        options.extensions = extensions.names
        options.platform = .apple
        options.drawsSelection = false
        options.overscan = Float(overscan)

        return options
    }

    /// A view with a new editor registering `extensions` (every StarterKit
    /// extension by default) and one empty paragraph.
    public convenience init(extensions: ExtensionSet = StarterKit().extensions) throws {
        let measurer = CoreTextMeasurer()
        let editor = try Editor(options: Self.editorOptions(extensions: extensions), measurer: measurer)
        self.init(editor: editor, measurer: measurer, extensions: extensions)
    }

    /// A view for an existing editor, made with ``editorOptions(extensions:)``
    /// and measuring with `measurer`.
    public init(editor: Editor, measurer: CoreTextMeasurer, extensions: ExtensionSet) {
        self.editor = editor
        self.measurer = measurer
        self.extensions = extensions
        canvas = CanvasView(painter: DisplayPainter(fonts: measurer.fonts))
        let relay = ChangeRelay()
        self.relay = relay
        observer = editor.addObserver(observer: relay)
        super.init(frame: .zero)

        relay.view = self
        setUp()
    }

    @available(*, unavailable)
    public required init?(coder: NSCoder) {
        nil
    }

    deinit {
        editor.removeObserver(id: observer)
    }

    private func setUp() {
        alwaysBounceVertical = true
        keyboardDismissMode = .interactive
        canvas.isUserInteractionEnabled = false
        addSubview(canvas)

        interaction.textInput = self
        addCheckboxGesture()
        addFocusGesture()
        installTextInteraction()
        observeKeyboard()
        setUpAccessibility()

        let themeTraits: [UITrait] = [UITraitUserInterfaceStyle.self, UITraitPreferredContentSizeCategory.self]
        registerForTraitChanges(themeTraits) { (view: CanvistView, _: UITraitCollection) in view.applyTheme() }
        applyTheme()
    }

    // MARK: Layout and painting
    override open func layoutSubviews() {
        super.layoutSubviews()
        paint()
    }

    /// Lay out the viewport and repaint what changed.
    private func paint() {
        positionCanvas()

        guard let frame = attempt("frame", editor.frame) else { return }

        resizeContent(to: CGFloat(frame.height))
        repaint(frame)

        if frame.shift != 0 {
            // Blocks above the viewport were measured: keep the content still.
            contentOffset.y += CGFloat(frame.shift)
        }

        if revealCaret {
            revealCaret = false
            self.scrollCaretToVisible()
        }
    }

    /// Tell the editor what is on screen, and cover it with the canvas.
    private func positionCanvas() {
        let width = max(bounds.width - textInsets.left - textInsets.right, 0)
        editor.setViewport(width: Float(width), height: Float(bounds.height))
        editor.setScroll(top: Float(bounds.minY - textInsets.top))
        canvas.frame = bounds
        canvas.origin = CGPoint(x: textInsets.left - bounds.minX, y: textInsets.top - bounds.minY)

        // UIKit puts the caret and selection views at the back; the opaque
        // page must stay behind them.
        if subviews.first !== canvas {
            sendSubviewToBack(canvas)
        }
    }

    private func resizeContent(to documentHeight: CGFloat) {
        let size = CGSize(width: bounds.width, height: documentHeight + textInsets.top + textInsets.bottom)

        if contentSize != size {
            contentSize = size
        }
    }

    private func repaint(_ frame: Frame) {
        let page = UIColor(frame.background)

        if canvas.page != page {
            canvas.page = page
            backgroundColor = page
        }

        guard let items = frame.items else { return }

        self.items = items
        canvas.items = items

        if frame.full {
            canvas.setNeedsDisplay()

            return
        }

        for damage in frame.damage {
            let rect = DisplayPainter.rect(damage).offsetBy(dx: canvas.origin.x, dy: canvas.origin.y)
            canvas.setNeedsDisplay(rect.insetBy(dx: -Self.damageMargin, dy: -Self.damageMargin))
        }
    }

    /// Scroll so the caret, or the selection's moving end, is on screen
    /// above the keyboard.
    func scrollCaretToVisible() {
        guard isFirstResponder, let caret = editor.caretRect(offset: editor.selection().head) else { return }

        scrollRectToVisible(viewRect(caret).insetBy(dx: 0, dy: -Self.caretMargin), animated: false)
    }

    // MARK: Changes
    /// The editor changed: repaint on the next layout pass, and tell the text
    /// input system about changes it did not make itself.
    func editorChanged(_ change: EditorChange) {
        if inputCalls == 0 {
            notifyInputSystem(change)
        } else if change.document || change.selection {
            revealCaret = true
        }

        setNeedsLayout()
        onChange?(change)
    }

    private func notifyInputSystem(_ change: EditorChange) {
        guard let delegate = textInputDelegate else { return }

        if change.document || change.composition {
            delegate.textWillChange(self)
            delegate.textDidChange(self)
        }

        if change.selection || change.document {
            delegate.selectionWillChange(self)
            delegate.selectionDidChange(self)
        }
    }

    /// Run `body` as a call from the text input system.
    func fromInput<T>(_ name: StaticString, _ body: () throws -> T) -> T? {
        inputCalls += 1
        defer { inputCalls -= 1 }

        return attempt(name, body)
    }

    /// Run `body`, logging why it failed. UIKit's text input calls cannot
    /// throw, and a failed edit must not stop the next keystroke.
    func attempt<T>(_ name: StaticString, _ body: () throws -> T) -> T? {
        do {
            return try body()
        } catch {
            logger.error("\(name, privacy: .public) failed: \(error.localizedDescription, privacy: .public)")

            return nil
        }
    }

    // MARK: Theme
    /// Follow the system's appearance and text size.
    private func applyTheme() {
        var theme = defaultThemeOptions()
        theme.dark = traitCollection.userInterfaceStyle == .dark
        let size = UIFontMetrics(forTextStyle: .body).scaledValue(for: baseFontSize, compatibleWith: traitCollection)
        theme.fontSize = Float(size)

        guard theme != self.theme else { return }

        self.theme = theme

        do {
            try editor.setTheme(theme: theme)
        } catch {
            logger.error("theme failed: \(error.localizedDescription, privacy: .public)")
        }

        canvas.painter.forgetLines()
        setNeedsLayout()
    }

    // MARK: Coordinates
    /// A rectangle in document coordinates, in this view's (scrolling)
    /// coordinates.
    func viewRect(_ bounds: Bounds) -> CGRect {
        DisplayPainter.rect(bounds).offsetBy(dx: textInsets.left, dy: textInsets.top)
    }

    /// A point in this view's coordinates, in document coordinates.
    func documentPoint(_ point: CGPoint) -> (x: Float, y: Float) {
        (Float(point.x - textInsets.left), Float(point.y - textInsets.top))
    }

    /// The default space between the view's edges and the text.
    public static let defaultTextInsets = UIEdgeInsets(top: 16, left: 20, bottom: 32, right: 20)
    /// The default body text size, UIKit's body text style at the default
    /// text size.
    public static let defaultFontSize: CGFloat = 17
    /// How far past the viewport to lay out.
    static let overscan: CGFloat = 400
    private static let damageMargin: CGFloat = 2
    private static let caretMargin: CGFloat = 12
}

/// The view that paints the display list. It covers the scroll view's
/// visible bounds and moves with them.
final class CanvasView: UIView {
    let painter: DisplayPainter
    var items: [DisplayItem] = []
    /// Where document coordinate (0, 0) is in this view.
    var origin = CGPoint.zero
    var page = UIColor.systemBackground

    init(painter: DisplayPainter) {
        self.painter = painter
        super.init(frame: .zero)
        isOpaque = true
        contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        nil
    }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }

        painter.paint(items, in: context, origin: origin, clip: rect, page: page)
    }
}

/// Forwards the editor's change notifications to the view on the main
/// thread. The editor holds it strongly and it holds the view weakly, so
/// the view and its editor never keep each other alive.
private final class ChangeRelay: EditorObserver, @unchecked Sendable {
    weak var view: CanvistView?

    func changed(change: EditorChange) {
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.deliver(change) }

            return
        }

        MainActor.assumeIsolated { deliver(change) }
    }

    @MainActor
    private func deliver(_ change: EditorChange) {
        view?.editorChanged(change)
    }
}
