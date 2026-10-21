// swift-tools-version: 6.0
//
// `Canvist` wraps the Rust core (`crates/canvist_ffi`) for Apple platforms.
// Run `build:ios` (or `./build.sh`) first: it produces the XCFramework and the
// generated `Sources/Canvist/Canvist.swift` that this package compiles.

import PackageDescription

let package = Package(
    name: "Canvist",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "Canvist", targets: ["Canvist"]),
    ],
    targets: [
        .binaryTarget(name: "CanvistFFI", path: "Frameworks/CanvistFFI.xcframework"),
        .target(name: "Canvist", dependencies: ["CanvistFFI"]),
        .testTarget(name: "CanvistTests", dependencies: ["Canvist"]),
    ]
)
