import UIKit

/// Shows `main.lynx.bundle`, the example's ReactLynx app, between the safe
/// area's top and the keyboard, with `@canvist/lynx`'s native module
/// registered. Lynx knows neither the safe area nor the keyboard, so the
/// host sizes its view.
final class ExampleViewController: UIViewController {
  private var lynxView: LynxView?

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .systemBackground

    let lynxView = LynxView { builder in
      let config = LynxConfig(provider: BundleProvider())
      config.register(CanvistLynxModule.self)
      builder.config = config
      builder.fontScale = 1.0
    }
    lynxView.layoutWidthMode = .exact
    lynxView.layoutHeightMode = .exact
    lynxView.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(lynxView)
    NSLayoutConstraint.activate([
      lynxView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
      lynxView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      lynxView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      lynxView.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
    ])
    self.lynxView = lynxView
    // `perfSlack` multiplies the benchmark's budgets on slow CI simulators;
    // Maestro passes it as a launch argument.
    let perfSlack = UserDefaults.standard.double(forKey: "perfSlack")
    let initData = perfSlack > 0 ? LynxTemplateData(dictionary: ["perfSlack": perfSlack]) : nil
    lynxView.loadTemplate(fromURL: "main.lynx", initData: initData)
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()

    guard let lynxView else { return }

    let size = lynxView.bounds.size

    guard
      lynxView.preferredLayoutWidth != size.width || lynxView.preferredLayoutHeight != size.height
    else {
      return
    }

    lynxView.preferredLayoutWidth = size.width
    lynxView.preferredLayoutHeight = size.height
    lynxView.triggerLayout()
  }
}

/// Reads Lynx bundles from the app's resources.
final class BundleProvider: NSObject, LynxTemplateProvider {
  func loadTemplate(withUrl url: String!, onComplete callback: LynxTemplateLoadBlock!) {
    guard let path = Bundle.main.path(forResource: url, ofType: "bundle") else {
      callback(
        nil,
        NSError(
          domain: "dev.canvist.lynx.example", code: 404,
          userInfo: [
            NSLocalizedDescriptionKey: "No bundle named \(url ?? "") in the app."
          ]))

      return
    }

    do {
      callback(try Data(contentsOf: URL(fileURLWithPath: path)), nil)
    } catch {
      callback(nil, error)
    }
  }
}
