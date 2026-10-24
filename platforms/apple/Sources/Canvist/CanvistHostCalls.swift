import Foundation

/// A JavaScript host's calls into the editors ``HostedEditors`` finds by
/// id, in the types Objective-C sees, for React Native's and Lynx's native
/// modules. A failure is an `NSError` whose description is
/// `[canvist:<code>] <reason>` (``HostedEditorError/message``).
@objc(CanvistHostCalls)
public final class CanvistHostCalls: NSObject {
  @objc(runWithId:name:args:error:)
  public static func run(id: String, name: String, args: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.run(command: Command(name: name, args: args))) }
  }

  @objc(canWithId:name:args:error:)
  public static func can(id: String, name: String, args: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.can(command: Command(name: name, args: args))) }
  }

  @objc(queryWithId:name:args:error:)
  public static func query(id: String, name: String, args: String) throws -> String {
    try call(id) { try $0.query(name: name, args: args) }
  }

  @objc(loadWithId:format:content:error:)
  public static func load(id: String, format: String, content: String) throws -> NSNumber {
    try call(id) { editor in
      try editor.load(format: try DocFormat(name: format), content: content)

      return NSNumber(value: true)
    }
  }

  @objc(exportWithId:format:error:)
  public static func export(id: String, format: String) throws -> String {
    try call(id) { $0.export(format: try DocFormat(name: format)) }
  }

  @objc(selectionWithId:error:)
  public static func selection(id: String) throws -> String {
    try call(id) { $0.selectionJSON() }
  }

  @objc(selectWithId:range:error:)
  public static func select(id: String, range: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.select(rangeJSON: range)) }
  }

  @objc(selectAllWithId:error:)
  public static func selectAll(id: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.selectAll()) }
  }

  @objc(insertTextWithId:text:error:)
  public static func insertText(id: String, text: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.insertText(text: text)) }
  }

  @objc(undoWithId:error:)
  public static func undo(id: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.undo()) }
  }

  @objc(redoWithId:error:)
  public static func redo(id: String) throws -> NSNumber {
    try call(id) { NSNumber(value: try $0.redo()) }
  }

  @objc(canUndoWithId:error:)
  public static func canUndo(id: String) throws -> NSNumber {
    try call(id) { NSNumber(value: $0.canUndo()) }
  }

  @objc(canRedoWithId:error:)
  public static func canRedo(id: String) throws -> NSNumber {
    try call(id) { NSNumber(value: $0.canRedo()) }
  }

  private static func call<T>(_ id: String, _ body: (Editor) throws -> T) throws -> T {
    do {
      return try HostedEditors.shared.call(id, body)
    } catch {
      throw NSError(
        domain: "dev.canvist.host",
        code: 1,
        userInfo: [NSLocalizedDescriptionKey: error.message]
      )
    }
  }
}
