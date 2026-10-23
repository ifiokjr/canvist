import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    // Lynx must start before any LynxView exists.
    LynxEnv.sharedInstance()

    let window = UIWindow(frame: UIScreen.main.bounds)
    window.rootViewController = ExampleViewController()
    window.makeKeyAndVisible()
    self.window = window

    return true
  }
}
