import SwiftUI

/// A ``CanvistView`` in SwiftUI.
///
/// Make the view once, in a model that outlives body updates, and run
/// commands on its ``CanvistView/editor``:
///
/// ```swift
/// @Observable final class Model {
///     let kit = StarterKit()
///     let view: CanvistView
///     init() throws { view = try CanvistView(extensions: kit.extensions) }
/// }
///
/// struct ContentView: View {
///     @State private var model = try! Model()
///     var body: some View {
///         CanvistEditor(model.view)
///     }
/// }
/// ```
public struct CanvistEditor: UIViewRepresentable {
    private let view: CanvistView

    /// Show `view`.
    public init(_ view: CanvistView) {
        self.view = view
    }

    public func makeUIView(context: Context) -> CanvistView {
        view
    }

    public func updateUIView(_ view: CanvistView, context: Context) {}
}
