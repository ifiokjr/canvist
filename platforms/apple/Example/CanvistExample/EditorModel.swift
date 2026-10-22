import Canvist
import Observation
import UIKit

/// One editor and the state its toolbar shows.
@MainActor
@Observable
final class EditorModel {
  let kit = StarterKit()
  let view: CanvistView
  private(set) var active: Set<Format> = []
  private(set) var canUndo = false
  private(set) var canRedo = false
  private(set) var failure: String?
  /// Called after every change, for collaboration.
  @ObservationIgnored var changed: ((EditorChange) -> Void)?

  init(markdown: String, label: String) throws {
    view = try CanvistView(extensions: kit.extensions)
    view.accessibilityLabel = label
    view.accessibilityIdentifier = "\(label.lowercased())-editor"
    try view.editor.load(format: .markdown, content: markdown)
    view.onChange = { [weak self] change in
      self?.refresh()
      self?.changed?(change)
    }
    refresh()
  }

  /// Run `format`'s command on the selection.
  func toggle(_ format: Format) {
    perform { _ = try view.editor.run(command: format.command(kit)) }
  }

  func undo() {
    perform { _ = try view.editor.undo() }
  }

  func redo() {
    perform { _ = try view.editor.redo() }
  }

  private func perform(_ body: () throws -> Void) {
    do {
      try body()
      failure = nil
    } catch {
      failure = error.localizedDescription
    }
  }

  /// Read the toolbar state back from the editor.
  private func refresh() {
    let editor = view.editor
    active = Set(Format.allCases.filter { (try? $0.isActive(kit, editor)) == true })
    canUndo = editor.canUndo()
    canRedo = editor.canRedo()
  }
}

/// The toolbar's formats, each a typed command and the query that says
/// whether it applies to the selection.
enum Format: String, CaseIterable, Identifiable {
  case bold, italic, underline, strike, heading, bulletList, taskList, quote, code

  var id: String { rawValue }

  var title: String {
    switch self {
    case .bold: "Bold"
    case .italic: "Italic"
    case .underline: "Underline"
    case .strike: "Strikethrough"
    case .heading: "Heading"
    case .bulletList: "Bulleted list"
    case .taskList: "Checklist"
    case .quote: "Quote"
    case .code: "Code block"
    }
  }

  var symbol: String {
    switch self {
    case .bold: "bold"
    case .italic: "italic"
    case .underline: "underline"
    case .strike: "strikethrough"
    case .heading: "textformat.size.larger"
    case .bulletList: "list.bullet"
    case .taskList: "checklist"
    case .quote: "text.quote"
    case .code: "chevron.left.forwardslash.chevron.right"
    }
  }

  func command(_ kit: StarterKit) -> Command {
    switch self {
    case .bold: kit.bold.toggleBold()
    case .italic: kit.italic.toggleItalic()
    case .underline: kit.underline.toggleUnderline()
    case .strike: kit.strike.toggleStrike()
    case .heading: kit.heading.toggleHeading(level: 2)
    case .bulletList: kit.bulletList.toggleBulletList()
    case .taskList: kit.taskList.toggleTaskList()
    case .quote: kit.blockquote.toggleBlockquote()
    case .code: kit.codeBlock.toggleCodeBlock()
    }
  }

  func isActive(_ kit: StarterKit, _ editor: Editor) throws -> Bool {
    switch self {
    case .bold: try editor.query(kit.bold.isBold())
    case .italic: try editor.query(kit.italic.isItalic())
    case .underline: try editor.query(kit.underline.isUnderline())
    case .strike: try editor.query(kit.strike.isStrike())
    case .heading: try editor.query(kit.heading.headingLevel()) != nil
    case .bulletList: try editor.query(kit.bulletList.isBulletList())
    case .taskList: try editor.query(kit.taskList.isTaskList())
    case .quote: try editor.query(kit.blockquote.isBlockquote())
    case .code: try editor.query(kit.codeBlock.isCodeBlock())
    }
  }
}

/// Two editors joined over Yjs in memory, as two devices would be over a
/// network.
@MainActor
final class CollaborationModel {
  let alice: EditorModel
  let bob: EditorModel

  init() throws {
    alice = try EditorModel(markdown: "# Shared notes\n\nType in either editor.", label: "Alice")
    bob = try EditorModel(markdown: "", label: "Bob")
    try alice.view.editor.share(clientId: 1)
    try bob.view.editor.join(state: alice.view.editor.encodeState(), clientId: 2)
    alice.changed = { [weak self] change in self?.send(change, from: \.alice, to: \.bob) }
    bob.changed = { [weak self] change in self?.send(change, from: \.bob, to: \.alice) }
  }

  /// Deliver `from`'s new Yjs updates to `to`.
  private func send(
    _ change: EditorChange,
    from: KeyPath<CollaborationModel, EditorModel>,
    to: KeyPath<CollaborationModel, EditorModel>
  ) {
    guard change.updates else { return }

    do {
      for update in try self[keyPath: from].view.editor.takeUpdates() {
        _ = try self[keyPath: to].view.editor.applyUpdate(update: update)
      }
    } catch {
      print("collaboration failed: \(error)")
    }
  }
}
