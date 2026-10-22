import UIKit

/// A ``CanvistView`` for a JavaScript host such as React Native or Lynx:
/// created from the host element's first props, registered with
/// ``HostedEditors`` under the host's id, and reporting what changed
/// through callbacks the host turns into events.
@objc(CanvistHostView)
@MainActor
public final class CanvistHostView: UIView {
  /// The editor is ready for calls under the id it was started with.
  @objc public var onReady: ((String) -> Void)?
  /// The document, selection, or composition changed.
  @objc public var onChange: ((Bool, Bool, Bool) -> Void)?
  /// The editor gained or lost focus.
  @objc public var onFocusChange: ((Bool) -> Void)?
  /// The editor could not start or load its content: a code and a reason.
  @objc public var onError: ((String, String) -> Void)?

  /// The body text size before Dynamic Type; 0 is the platform's default.
  @objc public var fontSize: CGFloat = 0 {
    didSet { applyFontSize() }
  }

  /// The label VoiceOver reads for the editor, which VoiceOver moves
  /// through line by line; this container is not an element of its own.
  override public var accessibilityLabel: String? {
    didSet { editorView?.accessibilityLabel = accessibilityLabel }
  }

  /// Never an element itself, whatever a host sets, so VoiceOver stops at
  /// the editor once rather than at the container and then the editor.
  override public var isAccessibilityElement: Bool {
    get { false }
    set {}
  }

  private var editorView: CanvistView?
  private var editorId: String?

  /// Create the editor, registering `extensions`, and load `content` as
  /// `format` (when both are given). Later calls do nothing: a view's
  /// editor is fixed once created.
  @objc public func start(editorId: String, extensions: [String], format: String?, content: String?)
  {
    guard editorView == nil else { return }

    let view: CanvistView

    do {
      view = try CanvistView(extensions: ExtensionSet(names: extensions))
    } catch {
      report(error)

      return
    }

    view.frame = bounds
    view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    view.accessibilityLabel = accessibilityLabel
    view.onChange = { [weak self] change in
      self?.onChange?(change.document, change.selection, change.composition)
    }
    view.onFocusChange = { [weak self] focused in self?.onFocusChange?(focused) }
    addSubview(view)
    editorView = view
    self.editorId = editorId
    applyFontSize()
    load(format: format, content: content, into: view.editor)
    HostedEditors.shared.register(view.editor, id: editorId)
    onReady?(editorId)
  }

  /// Unregister the editor and remove its view.
  @objc public func stop() {
    if let editorId {
      HostedEditors.shared.unregister(id: editorId)
    }

    editorView?.removeFromSuperview()
    editorView = nil
    editorId = nil
  }

  @objc public func focus() {
    editorView?.becomeFirstResponder()
  }

  @objc public func blur() {
    editorView?.resignFirstResponder()
  }

  private func load(format: String?, content: String?, into editor: Editor) {
    guard let format, let content else { return }

    do {
      try editor.load(format: try DocFormat(name: format), content: content)
    } catch {
      report(error)
    }
  }

  private func report(_ error: any Error) {
    let failure = HostedEditorError(error)
    onError?(failure.code, failure.reason)
  }

  private func applyFontSize() {
    editorView?.baseFontSize = fontSize > 0 ? fontSize : CanvistView.defaultFontSize
  }
}
