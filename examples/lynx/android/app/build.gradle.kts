plugins {
    id("com.android.application")
}

// The newest Lynx with published artifacts on every platform.
val lynxVersion = "4.1.0"

android {
    namespace = "dev.canvist.lynx.example"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.canvist.lynx.example"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"

        // canvist's engine is built for these ABIs only; `-PcanvistAbi` builds
        // for the one device under test.
        ndk {
            val abi = providers.gradleProperty("canvistAbi").orNull
            abiFilters += if (abi != null) listOf(abi) else listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            // An example: Release for its speed, signed with the debug key.
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":canvist-lynx"))
    implementation("org.lynxsdk.lynx:lynx:$lynxVersion")
    implementation("org.lynxsdk.lynx:lynx-jssdk:$lynxVersion")
    implementation("org.lynxsdk.lynx:lynx-trace:$lynxVersion")
    implementation("org.lynxsdk.lynx:primjs:4.1.1")
    implementation("androidx.core:core:1.17.0")
}
