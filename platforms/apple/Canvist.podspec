# The `Canvist` Swift package as a CocoaPod, for apps that build with
# CocoaPods: React Native (`@canvist/react-native`) and Lynx. Run `build.sh`
# first; it produces the XCFramework and the generated Swift bindings this
# pod compiles. Like the Swift package, it is not on a registry yet: point a
# Podfile at this directory with `pod "Canvist", path: ".../platforms/apple"`.

version = File.read(File.join(__dir__, "../../Cargo.toml"))[/^version = "([^"]+)"/, 1]

Pod::Spec.new do |s|
  s.name = "Canvist"
  s.version = version
  s.summary = "canvist's rich-text editor for iOS: CanvistView over canvist's Rust engine."
  s.homepage = "https://github.com/canvist/canvist/tree/main/platforms/apple"
  s.license = { type: "Unlicense" }
  s.author = { "Ifiok Jr." => "support@canvist.dev" }
  s.source = { git: "https://github.com/canvist/canvist.git", tag: "v#{s.version}" }

  s.platforms = { ios: "17.0" }
  s.swift_version = "6.0"
  s.source_files = "Sources/Canvist/**/*.swift"
  s.vendored_frameworks = "Frameworks/CanvistFFI.xcframework"
  s.frameworks = "UIKit", "CoreText", "UniformTypeIdentifiers"

  # The XCFramework's simulator slice is arm64 only (`build.sh`), so neither
  # this pod nor the app may build the Intel simulator architecture, which
  # Release simulator builds otherwise add.
  intel_simulator = { "EXCLUDED_ARCHS[sdk=iphonesimulator*]" => "x86_64" }
  s.pod_target_xcconfig = intel_simulator
  s.user_target_xcconfig = intel_simulator
end
