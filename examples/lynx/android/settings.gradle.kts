// The Android app that hosts the Lynx example. `e2e/test.sh android` copies the
// bundle in and builds it; `@canvist/lynx` builds from the package itself.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        google()
        mavenCentral()
        // `canvist-android`, published from `platforms/android` with
        // `./gradlew :canvist:publishToMavenLocal`, so the example runs against
        // the editor in the same checkout.
        mavenLocal {
            content { includeGroup("io.github.ifiokjr") }
        }
    }
}

rootProject.name = "canvist-lynx-example"
include(":app", ":canvist-lynx")
project(":canvist-lynx").projectDir = file("../../../packages/canvist-lynx/android")
