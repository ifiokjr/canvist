import Canvist
import SwiftUI

/// Types into a Rust-owned document and shows what Rust exports.
struct ContentView: View {
    @State private var draft = ""
    @State private var document = ""
    @State private var failure: String?
    @State private var editor = Result { try Editor(options: defaultEditorOptions(), measurer: CoreTextMeasurer()) }

    var body: some View {
        NavigationStack {
            Form {
                Section("Append") {
                    TextField("Text", text: $draft)
                        .accessibilityIdentifier("draft")
                        .onSubmit(append)
                    Button("Append", action: append)
                        .accessibilityIdentifier("append")
                        .disabled(draft.isEmpty)
                }

                Section("Document (from Rust)") {
                    Text(document.isEmpty ? "Empty" : document)
                        .foregroundStyle(document.isEmpty ? .secondary : .primary)
                        .accessibilityIdentifier("document")

                    if let failure {
                        Text(failure)
                            .foregroundStyle(.red)
                    }
                }

                Section {
                    LabeledContent("Rust core", value: coreVersion())
                }
            }
            .navigationTitle("Canvist")
        }
    }

    private func append() {
        guard !draft.isEmpty else { return }
        do {
            let editor = try editor.get()
            let end = editor.textLength()
            _ = try editor.select(anchor: end, head: end)
            _ = try editor.insertText(text: draft)
            document = editor.export(format: .markdown)
            draft = ""
            failure = nil
        } catch {
            failure = error.localizedDescription
        }
    }
}

#Preview {
    ContentView()
}
