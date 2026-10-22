require "json"

package = JSON.parse(File.read(File.join(__dir__, "package.json")))

Pod::Spec.new do |s|
  s.name = "CanvistLynx"
  s.version = package["version"]
  s.summary = package["description"]
  s.homepage = package["homepage"]
  s.license = { type: package["license"] }
  s.author = { "Ifiok Jr." => "support@canvist.dev" }
  s.source = { git: "https://github.com/canvist/canvist.git", tag: "v#{s.version}" }

  s.platforms = { ios: "17.0" }
  s.source_files = "ios/**/*.{h,m}"
  # Apps register the element by its tag and import only the module.
  s.public_header_files = "ios/CanvistLynxModule.h"

  # The native editor and its hosting (`CanvistHostView`, `CanvistHostCalls`):
  # `platforms/apple/Canvist.podspec` in the canvist repository, which the
  # app's Podfile points at.
  s.dependency "Canvist", s.version.to_s
  s.dependency "Lynx"

  # No Intel simulator slice, like `Canvist`, and `Canvist-Swift.h`, which a
  # static Swift pod does not put on its dependents' search paths.
  s.pod_target_xcconfig = {
    "EXCLUDED_ARCHS[sdk=iphonesimulator*]" => "x86_64",
    "HEADER_SEARCH_PATHS" => '"$(PODS_CONFIGURATION_BUILD_DIR)/Canvist/Swift Compatibility Header"',
  }
end
