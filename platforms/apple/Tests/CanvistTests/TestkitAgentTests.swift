// The testkit agent: the Rust testkit daemon's instruction stream, run
// against the real `Canvist` editor on this simulator.
//
// The daemon (crates/canvist_testkit/src/daemon.rs) serves one instruction
// at a time on GET /next; this test takes each, runs it against an
// `Editor`, and answers on POST /reply. The Kotlin agent
// (platforms/android/.../TestkitAgentTest.kt) and the loopback agent in
// the daemon do the same, which is what makes one test run on every
// platform.
//
// `TEST_RUNER_TESTKIT_SERVER` (xcodebuild passes TEST_RUNER_-prefixed
// variables through) says where the daemon is; without it, this test
// skips, so a plain `xcodebuild test` of the package runs as before.

@testable import Canvist
import Foundation
import XCTest

final class TestkitAgentTests: XCTestCase {
	/// One editor this agent is driving, with the state its ops need.
	private final class Session {
		let editor: Editor
		var marked: String = ""

		init(_ instruction: StartInstruction) throws {
			var options = defaultEditorOptions()
			options.extensions = instruction.extensions
			options.client = UInt32(instruction.client)
			options.platform = instruction.apple ? .apple : .other
			let editor = try Editor(options: options, measurer: CoreTextMeasurer())
			// A phone-sized viewport, as a view would give it: line motions
			// (Home, End, the vertical arrows) follow the laid-out lines, and
			// a width of zero wraps every grapheme onto a line of its own.
			editor.setViewport(width: 360, height: 720)

			if instruction.load.format == "yjs" {
				let state = Data(base64Encoded: instruction.load.content) ?? Data()
				try editor.join(state: state, clientId: UInt64(instruction.client))
			} else {
				try editor.load(format: Self.format(instruction.load.format), content: instruction.load.content)
			}

			self.editor = editor
		}

		/// A wire format name as the FFI's `DocFormat`.
		static func format(_ name: String) -> DocFormat {
			switch name {
			case "json": .json
			case "html": .html
			case "markdown": .markdown
			default: .text
			}
		}
	}

	/// One editor per session: parallel harnesses interleave their
	/// instructions, and each owns its editor.
	private var sessions: [Int: Session] = [:]

	/// Where the daemon said it listens: the environment, or the file the
	/// daemon's runner writes (xcodebuild does not pass environment
	/// variables through to Swift Package test runners).
	private static func daemonServer() -> String? {
		if let fromEnvironment = ProcessInfo.processInfo.environment["TESTKIT_SERVER"] {
			return fromEnvironment
		}

		let file = URL(fileURLWithPath: #filePath)
			.deletingLastPathComponent() // CanvistTests
			.deletingLastPathComponent() // Tests
			.deletingLastPathComponent() // apple
			.deletingLastPathComponent() // platforms
			.deletingLastPathComponent() // the repository
			.appendingPathComponent("target/canvist-testkit/ios.server")

		return try? String(contentsOf: file, encoding: .utf8)
	}

	func testRunsTheTestkitInstructions() throws {
		guard let server = Self.daemonServer(), !server.isEmpty else {
			throw XCTSkip("no TESTKIT_SERVER; the testkit daemon starts this test")
		}

		let agent = Agent(server: server)

		while true {
			switch try agent.next() {
			case .wait:
				Thread.sleep(forTimeInterval: 0.25)
			case .done:
				return
			case .run(let instruction):
				try agent.reply(execute(instruction))
			}
		}
	}

	/// Run one instruction against the session's editor.
	private func execute(_ instruction: Instruction) -> Reply {
		switch instruction {
		case .start(let start):
			do {
				sessions[start.session] = try Session(start)

				return Reply(id: start.id, ok: true, value: nil, error: nil)
			} catch {
				return Reply(id: start.id, ok: false, value: nil, error: "\(error)")
			}
		case .op(let op):
			guard let session = sessions[op.session] else {
				return Reply(id: op.id, ok: false, value: nil, error: "no editor for this session")
			}

			return run(session, op)
		case .read(let read):
			guard let session = sessions[read.session] else {
				return Reply(id: read.id, ok: false, value: nil, error: "no editor for this session")
			}

			return answer(session, read)
		case .close(let close):
			sessions.removeValue(forKey: close.session)

			return Reply(id: close.id, ok: true, value: nil, error: nil)
		case .wait, .done:
			return Reply(id: 0, ok: true, value: nil, error: nil)
		}
	}

	/// One operation, through the same FFI calls an app makes.
	private func run(_ session: Session, _ op: OpInstruction) -> Reply {
		do {
			let value: Any?

			switch op.op {
			case .load(let load):
				try session.editor.load(format: Session.format(load.format), content: load.content)
				value = nil
			case .typeText(let text):
				var changed = false

				for character in text {
					changed = try session.editor.insertText(text: String(character)) || changed
				}

				value = changed
			case .insertText(let text):
				value = try session.editor.insertText(text: text)
			case .press(let press):
				let modifiers = Modifiers(
					shift: press.shift, ctrl: press.ctrl, alt: press.alt, meta: press.meta
				)
				value = try session.editor.press(key: press.key, modifiers: modifiers)
			case .select(let select):
				let head = select.head ?? select.anchor
				value = try session.editor.selectPositions(
					anchor: DocPosition(path: select.anchor.path, offset: select.anchor.offset),
					head: DocPosition(path: head.path, offset: head.offset)
				)
			case .selectAll:
				value = try session.editor.selectAll()
			case .deleteSelection:
				// UIKit's text system deletes the selection with a
				// backspace; that is the input path here.
				value = try session.editor.press(
					key: "Backspace",
					modifiers: Modifiers(shift: false, ctrl: false, alt: false, meta: false)
				)
			case .paste:
				return Reply(
					id: op.id, ok: false, value: nil,
					error: "the iOS binding has no paste; test pasting on another target"
				)
			case .run(let call):
				value = try session.editor.run(
					command: Command(name: call.command, args: call.args ?? "null")
				)
			case .can(let call):
				value = try session.editor.can(
					command: Command(name: call.command, args: call.args ?? "null")
				)
			case .query(let call):
				let answer = try session.editor.query(name: call.query, args: call.args ?? "null")

				// Queries answer `true` as often as objects; fragments too.
				value = try JSONSerialization.jsonObject(
					with: Data(answer.utf8),
					options: [.fragmentsAllowed]
				)
			case .undo:
				value = try session.editor.undo()
			case .redo:
				value = try session.editor.redo()
			case .setMarked(let marked):
				value = try session.editor.setMarkedText(
					text: marked.text,
					selected: OffsetRange(
						start: UInt32(marked.selected.0), end: UInt32(marked.selected.1)
					)
				)
				session.marked = marked.text
			case .unmarkText:
				value = try session.editor.unmarkText()
				session.marked = ""
			case .share(let share):
				try session.editor.share(clientId: UInt64(share.client))
				value = nil
			case .applyUpdate(let update):
				let bytes = Data(base64Encoded: update.update) ?? Data()

				value = try session.editor.applyUpdate(update: bytes)
			case .takeUpdates:
				let updates = try session.editor.takeUpdates()

				value = updates.map { $0.base64EncodedString() }
			case .encodeState:
				value = try session.editor.encodeState().base64EncodedString()
			}

			return Reply(id: op.id, ok: true, value: value, error: nil)
		} catch {
			return Reply(id: op.id, ok: false, value: nil, error: "\(error)")
		}
	}

	/// Answer one read.
	private func answer(_ session: Session, _ read: ReadInstruction) -> Reply {
		let value: Any?

		switch read.what {
		case "text": value = session.editor.export(format: .text)
		case "markdown": value = session.editor.export(format: .markdown)
		case "html": value = session.editor.export(format: .html)
		case "json": value = session.editor.export(format: .json)
		case "selection":
			value = (session.editor.selectedPositions() ?? []).count >= 2
				? [
					"anchor": [
						"path": session.editor.selectedPositions()![0].path,
						"offset": session.editor.selectedPositions()![0].offset,
					],
					"head": [
						"path": session.editor.selectedPositions()![1].path,
						"offset": session.editor.selectedPositions()![1].offset,
					],
				]
				: nil
		case "canUndo": value = session.editor.canUndo()
		case "canRedo": value = session.editor.canRedo()
		default: value = nil
		}

		return Reply(id: read.id, ok: true, value: value, error: nil)
	}
}

// MARK: - The protocol, in the shapes wire.rs writes.

/// GET /next and POST /reply, over the daemon's tiny JSON HTTP.
private final class Agent {
	/// The daemon's base URL, without a trailing slash: paths join as
	/// written, so a query's `?` stays a query's `?`.
	private let server: String
	private let session = URLSession(configuration: .ephemeral)

	init(server: String) {
		self.server = server.hasSuffix("/") ? String(server.dropLast()) : server
	}

	/// One synchronous request: connect, write, read.
	private func request(_ method: String, _ path: String, _ body: Any?) throws -> Data {
		guard let url = URL(string: server + path) else {
			throw NSError(domain: "testkit", code: 3, userInfo: [
				NSLocalizedDescriptionKey: "bad daemon URL \(server)\(path)",
			])
		}

		var request = URLRequest(url: url)
		request.httpMethod = method
		request.timeoutInterval = 90

		if let body {
			request.setValue("application/json", forHTTPHeaderField: "Content-Type")
			request.httpBody = try JSONSerialization.data(withJSONObject: body)
		}

		var data: Data?
		var error: Error?
		let semaphore = DispatchSemaphore(value: 0)
		let task = session.dataTask(with: request) { received, _, failure in
			data = received
			error = failure
			semaphore.signal()
		}
		task.resume()
		semaphore.wait()

		if let error {
			throw error
		}

		guard let data, !data.isEmpty else {
			throw NSError(domain: "testkit", code: 1, userInfo: [
				NSLocalizedDescriptionKey: "empty reply from \(path)",
			])
		}

		return data
	}

	/// The next instruction, decoded.
	func next() throws -> Flow {
		let data = try request("GET", "/next?agent=ios", nil)
		let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]

		switch json["kind"] as? String {
		case "done":
			return .done
		case "wait", nil:
			return .wait
		default:
			return .run(try Instruction(json))
		}
	}

	/// Answer with one reply.
	func reply(_ reply: Reply) throws {
		_ = try request("POST", "/reply", reply.json)
	}
}

/// What the daemon said: work, patience, or the end.
private enum Flow {
	case wait
	case done
	case run(Instruction)
}

/// One instruction, from the daemon's JSON.
private enum Instruction {
	case start(StartInstruction)
	case op(OpInstruction)
	case read(ReadInstruction)
	case close(CloseInstruction)
	case wait
	case done

	init(_ json: [String: Any]) throws {
		let id = json["id"] as? Int ?? 0

		switch json["kind"] as? String {
		case "start":
			let load = json["load"] as? [String: Any] ?? [:]
			self = .start(StartInstruction(
				id: id,
				session: json["session"] as? Int ?? 0,
				extensions: (json["extensions"] as? [String]) ?? [],
				client: json["client"] as? Int ?? 1,
				apple: json["apple"] as? Bool ?? false,
				load: LoadInstruction(
					format: load["format"] as? String ?? "text",
					content: load["content"] as? String ?? ""
				)
			))
		case "op":
			self = .op(
				OpInstruction(
					id: id,
					session: json["session"] as? Int ?? 0,
					op: try Op(json["op"] as? [String: Any] ?? [:])
				)
			)
		case "read":
			self = .read(ReadInstruction(
				id: id, session: json["session"] as? Int ?? 0,
				what: json["what"] as? String ?? "text"
			))
		case "close":
			self = .close(CloseInstruction(id: id, session: json["session"] as? Int ?? 0))
		default:
			self = .wait
		}
	}
}

private struct StartInstruction {
	let id: Int
	let session: Int
	let extensions: [String]
	let client: Int
	let apple: Bool
	let load: LoadInstruction
}

private struct LoadInstruction {
	let format: String
	let content: String
}

private struct CloseInstruction {
	let id: Int
	let session: Int
}

private struct ReadInstruction {
	let id: Int
	let session: Int
	let what: String
}

private struct OpInstruction {
	let id: Int
	let session: Int
	let op: Op
}

/// One operation, from the daemon's JSON.
private enum Op {
	case load(LoadInstruction)
	case typeText(String)
	case insertText(String)
	case press(PressOp)
	case select(SelectOp)
	case selectAll
	case deleteSelection
	case paste
	case run(CallOp)
	case can(CallOp)
	case query(QueryOp)
	case undo
	case redo
	case setMarked(MarkedOp)
	case unmarkText
	case share(ShareOp)
	case applyUpdate(UpdateOp)
	case takeUpdates
	case encodeState

	init(_ json: [String: Any]) throws {
		let text = { (json["text"] as? String) ?? "" }

		switch json["type"] as? String {
		case "load":
			self = .load(LoadInstruction(
				format: json["format"] as? String ?? "text",
				content: json["content"] as? String ?? ""
			))
		case "typeText": self = .typeText(text())
		case "insertText": self = .insertText(text())
		case "press":
			self = .press(PressOp(
				key: json["key"] as? String ?? "",
				shift: json["shift"] as? Bool ?? false,
				ctrl: json["ctrl"] as? Bool ?? false,
				alt: json["alt"] as? Bool ?? false,
				meta: json["meta"] as? Bool ?? false
			))
		case "select":
			self = .select(SelectOp(
				anchor: Position(json["anchor"] as? [String: Any] ?? [:]),
				head: (json["head"] as? [String: Any]).map(Position.init)
			))
		case "selectAll": self = .selectAll
		case "deleteSelection": self = .deleteSelection
		case "paste": self = .paste
		case "run":
			self = .run(CallOp(
				command: json["command"] as? String ?? "",
				args: Op.arguments(json["args"])
			))
		case "can":
			self = .can(CallOp(
				command: json["command"] as? String ?? "",
				args: Op.arguments(json["args"])
			))
		case "query":
			self = .query(QueryOp(
				query: json["query"] as? String ?? "",
				args: Op.arguments(json["args"])
			))
		case "undo": self = .undo
		case "redo": self = .redo
		case "setMarkedText":
			let selected = json["selected"] as? [Int] ?? [0, 0]

			self = .setMarked(MarkedOp(text: text(), selected: (selected.first ?? 0, selected.last ?? 0)))
		case "unmarkText": self = .unmarkText
		case "share":
			self = .share(ShareOp(client: json["client"] as? Int ?? 1))
		case "applyUpdate":
			self = .applyUpdate(UpdateOp(update: json["update"] as? String ?? ""))
		case "takeUpdates": self = .takeUpdates
		case "encodeState": self = .encodeState
		default:
			throw NSError(domain: "testkit", code: 2, userInfo: [
				NSLocalizedDescriptionKey: "unknown op \(json)",
			])
		}
	}

	/// Command and query arguments, back to the JSON string the FFI takes.
	private static func arguments(_ value: Any?) -> String? {
		guard let value, !(value is NSNull) else {
			return nil
		}

		let data = try? JSONSerialization.data(withJSONObject: value)

		return data.map { String(decoding: $0, as: UTF8.self) }
	}
}

private struct PressOp {
	let key: String
	let shift: Bool
	let ctrl: Bool
	let alt: Bool
	let meta: Bool
}

private struct SelectOp {
	let anchor: Position
	let head: Position?
}

private struct CallOp {
	let command: String
	let args: String?
}

private struct QueryOp {
	let query: String
	let args: String?
}

private struct MarkedOp {
	let text: String
	let selected: (Int, Int)
}

private struct ShareOp {
	let client: Int
}

private struct UpdateOp {
	let update: String
}

/// A {path, offset} position.
private struct Position {
	let path: [UInt32]
	let offset: UInt32

	init(_ json: [String: Any]) {
		self.path = (json["path"] as? [Int] ?? []).map(UInt32.init)
		self.offset = UInt32(json["offset"] as? Int ?? 0)
	}
}

/// One reply, in the shape wire.rs reads.
private struct Reply {
	let id: Int
	let ok: Bool
	let value: Any?
	let error: String?

	var json: [String: Any] {
		[
			"id": id,
			"ok": ok,
			"value": value ?? NSNull(),
			"error": error ?? NSNull(),
		]
	}
}
