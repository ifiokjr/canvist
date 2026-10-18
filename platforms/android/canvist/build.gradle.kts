// The Kotlin binding for the Rust core (`crates/canvist_ffi`).
//
// Run `build:android` (or `./build.sh`) first: it cross-compiles
// `libcanvist_ffi.so` into `src/main/jniLibs` with cargo-ndk and generates the
// UniFFI Kotlin sources into `src/main/kotlin`.
plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "io.github.ifiokjr.canvist"
    compileSdk = 37

    defaultConfig {
        minSdk = 24
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    // `-PcanvistTestBuildType=release` runs the instrumented tests against an
    // optimised, non-debuggable build, which is what the FFI performance
    // numbers should come from.
    testBuildType = providers.gradleProperty("canvistTestBuildType").getOrElse("debug")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // UniFFI's Kotlin bindings call into Rust through JNA.
    api("${libs.jna.get()}@aar")
    implementation(libs.androidx.annotation)

    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.test.runner)
}
