// File: ./android/app/build.gradle.kts
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// --- HELPER: Read version from Cargo.toml ---
fun getCargoVersion(): String {
    val cargoFile = File(project.rootDir.parentFile, "Cargo.toml")
    if (!cargoFile.exists()) {
        println("Cargo.toml not found at $cargoFile, defaulting to 0.0.1")
        return "0.0.1"
    }

    cargoFile.useLines { lines ->
        for (line in lines) {
            val trimmed = line.trim()
            if (trimmed.startsWith("version") && trimmed.contains("=")) {
                val versionValue = trimmed.split("=")[1].trim().replace("\"", "")
                return versionValue
            }
        }
    }
    
    println("Version not found in Cargo.toml, defaulting to 0.0.1")
    return "0.0.1"
}

fun getVersionCode(versionName: String): Int {
    val parts = versionName.split(".").map { it.toIntOrNull() ?: 0 }
    if (parts.size >= 3) {
        return parts[0] * 10000 + parts[1] * 100 + parts[2]
    }
    return 1
}

val appVersionName = getCargoVersion()
val appVersionCode = getVersionCode(appVersionName)

println("Cfait Android Build: v$appVersionName (Code: $appVersionCode)")

android {
    namespace = "com.cfait"
    compileSdk = 36

    signingConfigs {
        create("release") {
            // Read from environment variables set by the CI
            val storeFile = System.getenv("KEYSTORE_FILE")
            val storePassword = System.getenv("KEYSTORE_PASSWORD")
            val keyAlias = System.getenv("KEY_ALIAS")
            val keyPassword = System.getenv("KEY_PASSWORD")
            
            if (storeFile != null && File(storeFile).exists()) {
                storeFile(File(storeFile))
                storePassword(storePassword)
                keyAlias(keyAlias)
                keyPassword(keyPassword)
            } else {
                // Allows local `assembleRelease` to run without signing (for testing)
                println("Signing config not found. Building unsigned release artifact.")
            }
        }
    }
    // ------------------------------------

    defaultConfig {
        applicationId = "com.cfait"
        minSdk = 23
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir("src/main/jniLibs")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true // Recommended for release
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Tell the 'release' build to use your new signing config
            signingConfig = signingConfigs.getByName("release")
        }
    }
    // ---------------------------------
    
    buildFeatures {
        compose = true
        buildConfig = true
    }
    
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
}

// Fix 'jvmTarget' deprecation warning using the new DSL
kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.10.0")
    implementation("androidx.activity:activity-compose:1.12.1")
    implementation(platform("androidx.compose:compose-bom:2025.12.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.navigation:navigation-compose:2.9.6")
    
    // Required for UniFFI
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
}

tasks.register<Copy>("copyFonts") {
    description = "Copies fonts from root assets to Android resources"
    from("${project.rootDir}/../assets/fonts/SymbolsNerdFont-Regular.ttf")
    into("${project.projectDir}/src/main/res/font")
    rename { "symbols_nerd_font.ttf" }
}

tasks.named("preBuild") {
    dependsOn("copyFonts")
}