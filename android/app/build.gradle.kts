plugins {
    id("com.android.application")
}

android {
    namespace = "app.assistant"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.assistant"
        minSdk = 31
        targetSdk = 36
        // Each build counts up, so a new APK installs over the old one.
        val build = (System.getenv("APP_VERSION_CODE") ?: "1").toInt()
        versionCode = build
        versionName = "0.1.$build"
        // Where the app connects first. Change it in the app or build with -PserverUrl=...
        val server = providers.gradleProperty("serverUrl").orElse("http://localhost:8790").get()
        buildConfigField("String", "DEFAULT_SERVER", "\"$server\"")
    }

    // A key of our own, kept on the build server, so each new build installs over the last one.
    signingConfigs {
        create("release") {
            storeFile = file(System.getenv("APP_KEYSTORE") ?: "release.keystore")
            storePassword = System.getenv("APP_KEYSTORE_PASS") ?: ""
            keyAlias = "assistant"
            keyPassword = System.getenv("APP_KEYSTORE_PASS") ?: ""
        }
    }
    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }

    buildFeatures { buildConfig = true }

    // Pinned. Health Connect reads body numbers on the phone; coroutines run its suspend calls.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation("androidx.health.connect:connect-client:1.1.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // Reads text in photos on the phone; the model is bundled in the app (no Google services, no network).
    implementation("com.google.mlkit:text-recognition:16.0.1")
    // FileProvider: hands the camera app a file to save the photo in.
    implementation("androidx.core:core:1.13.1")
}

// The screens are the web page in the AssistantX repo (examples/dummy-client/index.html).
// When that folder sits next to this project, building copies the latest page in.
val copyWeb = tasks.register<Copy>("copyWeb") {
    val src = rootProject.file("../AssistantX/examples/dummy-client/index.html")
    onlyIf { src.exists() }
    from(src)
    into(layout.projectDirectory.dir("src/main/assets"))
}
tasks.named("preBuild") { dependsOn(copyWeb) }
