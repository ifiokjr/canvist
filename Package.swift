// swift-tools-version: 6.0
//
// The root `Canvist` package: the Swift Package Manager only resolves
// `Package.swift` at the repository root, so this manifest is what Xcode
// resolves when someone adds https://github.com/canvist/canvist as a package
// dependency. It compiles the same sources as `platforms/apple/Package.swift`,
// which remains the local-development spelling of the same package.
//
// Run `platforms/apple/build.sh` (or `build:ios`) first: it produces the
// XCFramework and the generated `Sources/Canvist/Canvist.swift` that this
// package compiles.

import PackageDescription

let package = Package(
  name: "Canvist",
  platforms: [.iOS(.v17)],
  products: [
    .library(name: "Canvist", targets: ["Canvist"])
  ],
  targets: [
    .binaryTarget(
      name: "CanvistFFI",
      path: "platforms/apple/Frameworks/CanvistFFI.xcframework"
    ),
    .target(
      name: "Canvist",
      dependencies: ["CanvistFFI"],
      path: "platforms/apple/Sources/Canvist"
    ),
    .testTarget(
      name: "CanvistTests",
      dependencies: ["Canvist"],
      path: "platforms/apple/Tests/CanvistTests"
    ),
  ]
)
