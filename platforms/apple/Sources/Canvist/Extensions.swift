import Foundation

/// A command argument's value, as the generated handles pass them.
public enum ArgumentValue: Sendable, Hashable {
  case bool(Bool)
  case int(Int)
  case double(Double)
  case string(String)

  /// The value as JSON. Infinities and NaN, which JSON lacks, become
  /// `null`, which the command rejects.
  var json: String {
    switch self {
    case .bool(let flag): flag ? "true" : "false"
    case .int(let number): String(number)
    case .double(let number): number.isFinite ? String(number) : "null"
    case .string(let text): jsonString(text)
    }
  }
}

/// The command `name` with `args`. Arguments that are `nil` are left out,
/// so the command uses its default for them.
func command(_ name: String, _ args: [String: ArgumentValue?] = [:]) -> Command {
  let fields =
    args
    .compactMapValues { $0 }
    .sorted { $0.key < $1.key }
    .map { "\(jsonString($0.key)):\($0.value.json)" }

  return Command(name: name, args: "{\(fields.joined(separator: ","))}")
}

/// `text` as a JSON string literal.
func jsonString(_ text: String) -> String {
  var out = "\""

  for scalar in text.unicodeScalars {
    switch scalar {
    case "\"": out += "\\\""
    case "\\": out += "\\\\"
    case "\n": out += "\\n"
    case "\r": out += "\\r"
    case "\t": out += "\\t"
    case _ where scalar.value < 0x20: out += String(format: "\\u%04x", scalar.value)
    default: out.unicodeScalars.append(scalar)
    }
  }

  return out + "\""
}

/// A query from a typed handle, answered by ``Editor/query(_:)``.
public struct Query<Value>: Sendable {
  /// The query's name in the extension metadata.
  public let name: String
  let decode: @Sendable (Any) throws -> Value

  init(_ name: String, decode: @escaping @Sendable (Any) throws -> Value) {
    self.name = name
    self.decode = decode
  }
}

/// Decoders from the JSON queries answer with.
enum Decode {
  static func bool(_ json: Any) throws -> Bool {
    guard let value = json as? Bool else { throw mismatch("a boolean", json) }

    return value
  }

  static func int(_ json: Any) throws -> Int {
    guard let value = json as? Int else { throw mismatch("an integer", json) }

    return value
  }

  static func double(_ json: Any) throws -> Double {
    guard let value = json as? Double else { throw mismatch("a number", json) }

    return value
  }

  static func string(_ json: Any) throws -> String {
    guard let value = json as? String else { throw mismatch("a string", json) }

    return value
  }

  static func optional<Value>(
    _ decode: @escaping @Sendable (Any) throws -> Value
  ) -> @Sendable (Any) throws -> Value? {
    { json in json is NSNull ? nil : try decode(json) }
  }

  private static func mismatch(_ expected: String, _ json: Any) -> CanvistError {
    .Failed(code: "parse", reason: "expected \(expected), not \(json)")
  }
}

/// An extension to register with ``ExtensionSet/add(_:)``, which returns
/// its typed handle. ``Extensions`` lists the StarterKit's.
public struct Extension<Handle: Sendable>: Sendable {
  /// The extension's name in the metadata.
  public let name: String
  let handle: Handle
}

/// The extensions an editor registers, collected while handing out their
/// typed handles: a handle's commands exist whenever its extension does.
///
/// ```swift
/// var extensions = ExtensionSet()
/// let bold = extensions.add(Extensions.bold)
/// let view = CanvistView(extensions: extensions)
/// try view.editor.run(command: bold.toggleBold())
/// ```
public struct ExtensionSet: Sendable, Hashable {
  /// The registered extensions' names, in registration order.
  public private(set) var names: [String] = []

  /// An empty set: paragraphs and the base key bindings only.
  public init() {}

  /// The extensions `names` names, for hosts that choose them at run time,
  /// such as React Native and Lynx. There are no handles: run commands by
  /// name. An editor fails to start with a name it does not know.
  public init(names: [String]) {
    for name in names where !self.names.contains(name) {
      self.names.append(name)
    }
  }

  /// Register `extension` and return its handle.
  public mutating func add<Handle>(_ extension: Extension<Handle>) -> Handle {
    if !names.contains(`extension`.name) {
      names.append(`extension`.name)
    }

    return `extension`.handle
  }
}

extension Editor {
  /// Answer a query from a typed handle, such as `kit.heading.headingLevel()`.
  public func query<Value>(_ query: Query<Value>) throws -> Value {
    let answer = try self.query(name: query.name, args: "{}")
    let json = try JSONSerialization.jsonObject(
      with: Data(answer.utf8),
      options: .fragmentsAllowed
    )

    return try query.decode(json)
  }
}
