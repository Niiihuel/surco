import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ValueSource
import org.gradle.api.provider.ValueSourceParameters

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// The app's version is the workspace's, so the APK and the Rust library never disagree.
val surcoVersion = rootProject.file("../Cargo.toml").readText()
    .substringAfter("[workspace.package]")
    .substringBefore("\n[")
    .let { Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(it)?.groupValues?.get(1) }
    ?: error("no version under [workspace.package] in Cargo.toml")
val (versionMajor, versionMinor, versionPatch) = surcoVersion.split('.').map { it.toInt() }

android {
    namespace = "io.github.niiihuel.surco"
    compileSdk = 35

    defaultConfig {
        // Forks change this with -Psurco.applicationId; the Kotlin package stays, Rust finds it by name.
        applicationId = providers.gradleProperty("surco.applicationId").getOrElse("io.github.niiihuel.surco")
        minSdk = 26
        targetSdk = 35
        versionCode = versionMajor * 10000 + versionMinor * 100 + versionPatch
        versionName = surcoVersion
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation("androidx.games:games-activity:4.4.0")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.media3:media3-session:1.6.1")
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
    // Unversioned here: the version is the one Cargo.lock pins, see below.
    implementation("org.rustls:rustls-platform-verifier")
}

// From rustls-platform-verifier's README, Kotlin DSL. The README's `requested.group` check says
// "org.rustls", which is the group the artifact is published under.
abstract class RustlsVersion : ValueSource<String, RustlsVersion.Params> {
    interface Params : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    companion object {
        const val CRATE_NAME = "rustls-platform-verifier-android"
    }

    override fun obtain(): String {
        val version = parameters.lockFile.get().asFile.readLines().let { lines ->
            val nameIdx = lines.indexOfFirst { it.trim() == "name = \"$CRATE_NAME\"" }
            if (nameIdx < 0) {
                null
            } else {
                lines.drop(nameIdx + 1)
                    .firstOrNull { it.trimStart().startsWith("version = ") }
                    ?.substringAfter('"', "")
                    ?.substringBefore('"', "")
                    ?.takeIf { it.isNotEmpty() }
            }
        }
        return version ?: error("$CRATE_NAME not found in Cargo.lock")
    }
}

val rustlsPlatformVerifierVersion = providers.of(RustlsVersion::class.java) {
    parameters.lockFile.set(rootProject.layout.projectDirectory.file("../Cargo.lock"))
}

configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.rustls" && requested.name == "rustls-platform-verifier") {
            useVersion(rustlsPlatformVerifierVersion.get())
            because("native component version must be identical to version of ${RustlsVersion.CRATE_NAME}")
        }
    }
}
