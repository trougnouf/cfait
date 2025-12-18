// File: ./android/app/build.gradle.kts
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// --- HELPER: Read version info from Cargo.toml ---
fun getCargoVersionInfo(): Pair<String, Int> {
    val cargoFile = File(project.rootDir.parentFile, "Cargo.toml")
    var version = "0.0.1"
    var code = 1

    if (cargoFile.exists()) {
        cargoFile.useLines { lines ->
            for (line in lines) {
                val trimmed = line.trim()
                // Parse Version String
                if (trimmed.startsWith("version") && trimmed.contains("=")) {
                    version = trimmed.split("=")[1].trim().replace("\"", "")
                }
                // Parse Version Code (New logic)
                if (trimmed.startsWith("version_code") && trimmed.contains("=")) {
                    val codeStr = trimmed.split("=")[1].trim()
                    code = codeStr.toIntOrNull() ?: 1
                }
            }
        }
    }
    return Pair(version, code)
}

val (appVersionName, appVersionCode) = getCargoVersionInfo()

println("Cfait Android Build: v$appVersionName (Code: $appVersionCode)")

android {
    namespace = "com.cfait"
    compileSdk = 36

signingConfigs {
        create("release") {
            // Read from environment variables set by the CI
            val envStoreFile = System.getenv("KEYSTORE_FILE")
            val envStorePassword = System.getenv("KEYSTORE_PASSWORD")
            val envKeyAlias = System.getenv("KEY_ALIAS")
            val envKeyPassword = System.getenv("KEY_PASSWORD")
            
            if (envStoreFile != null && File(envStoreFile).exists()) {
                storeFile = File(envStoreFile)
                storePassword = envStorePassword
                keyAlias = envKeyAlias
                keyPassword = envKeyPassword
            } else {
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