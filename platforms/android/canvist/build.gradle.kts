// The Kotlin binding for the Rust core (`crates/canvist_ffi`).
//
// Run `build:android` (or `./build.sh`) first: it cross-compiles
// `libcanvist_ffi.so` into `src/main/jniLibs` with cargo-ndk and generates the
// UniFFI Kotlin sources into `src/main/kotlin`.
plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.maven.publish)
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

// Apps on older Kotlin compilers must be able to read this library: React
// Native 0.88 builds with Kotlin 2.2. Compile for the 2.2 language and API,
// against the 2.2 standard library.
kotlin {
    coreLibrariesVersion = "2.2.0"
    compilerOptions {
        languageVersion = org.jetbrains.kotlin.gradle.dsl.KotlinVersion.KOTLIN_2_2
        apiVersion = org.jetbrains.kotlin.gradle.dsl.KotlinVersion.KOTLIN_2_2
    }
}

dependencies {
    // UniFFI's Kotlin bindings call into Rust through JNA.
    api("${libs.jna.get()}@aar")
    implementation(libs.androidx.annotation)

    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.androidx.test.runner)
}

// Published to Maven Central as `io.github.ifiokjr:canvist-android` by the
// release workflow. monochange keeps `VERSION_NAME` in `gradle.properties` in
// step with the rest of the SDK.
mavenPublishing {
    publishToMavenCentral()
    // CI passes the key as `ORG_GRADLE_PROJECT_signingInMemoryKey`; local
    // `publishToMavenLocal` runs stay unsigned.
    if (providers.gradleProperty("signingInMemoryKey").isPresent) {
        signAllPublications()
    }
    coordinates("io.github.ifiokjr", "canvist-android", providers.gradleProperty("VERSION_NAME").get())
    pom {
        name = "canvist"
        description = "Android binding for canvist, the canvas-based rich-text editor with a Rust core."
        inceptionYear = "2026"
        url = "https://github.com/canvist/canvist"
        licenses {
            license {
                name = "The Unlicense"
                url = "https://unlicense.org"
                distribution = "repo"
            }
        }
        developers {
            developer {
                id = "ifiokjr"
                name = "Ifiok Jr."
                url = "https://github.com/ifiokjr"
            }
        }
        scm {
            url = "https://github.com/canvist/canvist"
            connection = "scm:git:git://github.com/canvist/canvist.git"
            developerConnection = "scm:git:ssh://git@github.com/canvist/canvist.git"
        }
    }
}
