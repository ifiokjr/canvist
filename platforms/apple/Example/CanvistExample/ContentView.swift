import Canvist
import SwiftUI

/// The example: a rich-text editor with a formatting toolbar, and two
/// editors collaborating.
struct ContentView: View {
    @State private var editor = Result { try EditorModel(markdown: initialMarkdown, label: "Editor") }
    @State private var collaboration = Result { try CollaborationModel() }

    var body: some View {
        TabView {
            Tab("Editor", systemImage: "doc.richtext") {
                Loaded(editor) { model in EditorScreen(model: model) }
            }

            Tab("Collaborate", systemImage: "person.2") {
                Loaded(collaboration) { model in CollaborationScreen(model: model) }
            }
        }
    }
}

/// `content` for a model that loaded, or why it did not.
struct Loaded<Model, Content: View>: View {
    let result: Result<Model, Error>
    let content: (Model) -> Content

    init(_ result: Result<Model, Error>, @ViewBuilder content: @escaping (Model) -> Content) {
        self.result = result
        self.content = content
    }

    var body: some View {
        switch result {
        case .success(let model): content(model)
        case .failure(let error): Text(error.localizedDescription)
        }
    }
}

/// Spacing and sizes the example's chrome uses.
private enum Metrics {
    static let button = CGSize(width: 36, height: 32)
    static let buttonRadius: CGFloat = 8
    static let activeOpacity = 0.18
    static let barPadding = EdgeInsets(top: 6, leading: 12, bottom: 6, trailing: 12)
    static let labelPadding = EdgeInsets(top: 12, leading: 20, bottom: 0, trailing: 20)
}

/// One editor under its toolbar.
struct EditorScreen: View {
    let model: EditorModel

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                FormatBar(model: model)
                Divider()
                CanvistEditor(model.view)
                    .ignoresSafeArea(.container, edges: .bottom)
            }
            .navigationTitle("Canvist")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button("Undo", systemImage: "arrow.uturn.backward", action: model.undo)
                        .disabled(!model.canUndo)
                        .accessibilityIdentifier("undo")
                    Button("Redo", systemImage: "arrow.uturn.forward", action: model.redo)
                        .disabled(!model.canRedo)
                        .accessibilityIdentifier("redo")
                }
            }
        }
    }
}

/// Toggles for the toolbar's formats, lit while the selection has them.
struct FormatBar: View {
    let model: EditorModel

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(Format.allCases) { format in
                    let active = model.active.contains(format)
                    Button {
                        model.toggle(format)
                    } label: {
                        Image(systemName: format.symbol)
                            .frame(width: Metrics.button.width, height: Metrics.button.height)
                            .background(
                                active ? Color.accentColor.opacity(Metrics.activeOpacity) : .clear,
                                in: .rect(cornerRadius: Metrics.buttonRadius)
                            )
                    }
                    .accessibilityLabel(format.title)
                    .accessibilityIdentifier(format.rawValue)
                    .accessibilityAddTraits(active ? .isSelected : [])
                }
            }
            .padding(Metrics.barPadding)
        }
    }
}

/// Two editors joined over Yjs.
struct CollaborationScreen: View {
    let model: CollaborationModel

    var body: some View {
        VStack(spacing: 0) {
            peer("Alice", model.alice)
            Divider()
            peer("Bob", model.bob)
        }
    }

    private func peer(_ name: String, _ editor: EditorModel) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(name)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(Metrics.labelPadding)
            CanvistEditor(editor.view)
        }
    }
}

/// UI tests start from an empty document.
private let initialMarkdown = ProcessInfo.processInfo.arguments.contains("-emptyDocument") ? "" : welcome

private let welcome = """
# Welcome to canvist

This text is laid out by **Rust** and painted with *Core Graphics*. \
Select it, format it, and undo it like any other text on iOS.

- [x] Type with the keyboard, dictation, or an input method
- [ ] Try `##` and a space for a heading

> Every change is a step that collaborators merge.
"""
