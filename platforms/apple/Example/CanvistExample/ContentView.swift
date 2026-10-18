import Canvist
import SwiftUI

/// Appends text to a Rust-owned document and renders what Rust returns.
struct ContentView: View {
    @State private var editor = EditorHandle()
    @State private var draft = ""
    @State private var document = ""
    @State private var failure: String?

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
                    LabeledContent("Rust core", value: editor.version())
                }
            }
            .navigationTitle("Canvist")
        }
    }

    private func append() {
        guard !draft.isEmpty else { return }
        do {
            // The placeholder core counts offsets in Unicode scalar values.
            let end = UInt32(document.unicodeScalars.count)
            try editor.insertText(offset: end, text: draft)
            document = editor.text()
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
