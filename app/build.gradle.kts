plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "com.transfer.fork"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.transfer.fork"
        minSdk = 29
        targetSdk = 35
        versionCode = 601
        versionName = "0.6.1"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    buildFeatures {
        viewBinding = true
        buildConfig = true // make it possible to import BuildConfig for VERSION_NAME
    }

    // Rust core: built by cargo-ndk so `./gradlew assembleDebug` stays a single command.
    val cargo = file("${System.getProperty("user.home")}/.cargo/bin/cargo")
    val rustCore = file("../transfer-core")
    val cargoNdk = tasks.register<Exec>("cargoNdk") {
        workingDir = rustCore
        environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME") ?: "/opt/android-ndk")
        commandLine(
            cargo.absolutePath, "ndk",
            "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
            "-o", "../app/src/main/jniLibs",
            "build", "--release",
        )
        inputs.dir(file("../transfer-core/src"))
        inputs.file(file("../transfer-core/Cargo.toml"))
        outputs.dir(file("src/main/jniLibs"))
    }
    tasks.named("preBuild") { dependsOn(cargoNdk) }

    signingConfigs {
        create("release") {
            storeFile = file("keystore/transfer-key.jks")
            storePassword = System.getenv("KEYSTORE_PASSWORD")
            keyAlias = System.getenv("KEY_ALIAS")
            keyPassword = System.getenv("KEY_PASSWORD")
        }
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = signingConfigs.getByName("release")

        }
    }

    //  metadata encrypted with a public key belonging to Google. see  https://android.izzysoft.de/articles/named/iod-scan-apkchecks#blobs
    dependenciesInfo {
        // Disables dependency metadata when building APKs (for IzzyOnDroid/F-Droid)
        includeInApk = false
        // Disables dependency metadata when building Android App Bundles (for Google Play)
        includeInBundle = false
    }

}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation(libs.androidx.constraintlayout)
    implementation(libs.androidx.navigation.fragment.ktx)
    implementation(libs.androidx.navigation.ui.ktx)

    implementation(libs.androidx.preference.ktx)
    implementation(libs.timber)
    implementation(libs.androidx.activity)


    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)

    androidTestImplementation (libs.androidx.runner)
    androidTestImplementation (libs.androidx.rules)

    // Espresso
    androidTestImplementation (libs.androidx.espresso.contrib)

    // UI Automator
    androidTestImplementation (libs.androidx.uiautomator)

    // JSON parsing in the instrumented tests (Ktor used to provide it transitively)
    androidTestImplementation (libs.kotlinx.serialization.json)

    // OkHttp for making network requests in the test
    androidTestImplementation (libs.okhttp)

    androidTestImplementation (libs.awaitility.kotlin)

    // QR Code generation
    implementation(libs.zxing.core)
    implementation(libs.zxing.android.embedded)

}