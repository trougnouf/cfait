// SPDX-License-Identifier: GPL-3.0-or-later
// Android Application class initializing the Rust backend.
package com.trougnouf.cfait

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.os.Build
import android.util.Log
import com.trougnouf.cfait.core.CfaitMobile
import kotlinx.coroutines.CompletableDeferred
import kotlin.concurrent.thread

class CfaitApplication : Application() {
    lateinit var api: CfaitMobile
        private set

    // Completed once the background cache load finishes (success or failure),
    // so background workers can wait for it instead of racing an empty store.
    val dataLoaded = CompletableDeferred<Unit>()

    // Declare the external function
    private external fun initNdkContext(context: android.content.Context)

    companion object {
        init {
            // Explicitly load the native library
            System.loadLibrary("cfait")
        }
    }

    override fun onCreate() {
        super.onCreate()

        // Set up crash logging to capture JVM crashes in the debug export zip
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, exception ->
            try {
                val crashFile = java.io.File(cacheDir, "android_crash.txt")
                crashFile.appendText("\n--- CRASH at ${java.util.Date()} ---\n")
                val sw = java.io.StringWriter()
                exception.printStackTrace(java.io.PrintWriter(sw))
                crashFile.appendText(sw.toString())
            } catch (e: Exception) {
                // Ignore errors during crash handling
            }
            defaultHandler?.uncaughtException(thread, exception)
        }

        // 1. Initialize the NDK context FIRST
        initNdkContext(this)

        // 2. Now perform the rest of the initialization
        com.trougnouf.cfait.core.initTokioRuntime()
        api = CfaitMobile(filesDir.absolutePath)

        // Create notification channel once at app startup (Android O+)
        // This avoids redundant IPC overhead on every alarm fire
        createNotificationChannel()

        // Preload data into memory on a background thread so the FFI call
        // never blocks the main thread; waiters await dataLoaded.
        thread {
            try {
                api.loadFromCache()
            } catch (e: Throwable) {
                Log.e("CfaitApp", "Failed to load cache", e)
            } finally {
                dataLoaded.complete(Unit)
            }
        }

        // Detect saved language preference or fall back to Android system language,
        // then propagate it to the Rust backend so rust_i18n is initialized correctly.
        val prefs = getSharedPreferences("cfait_prefs", android.content.Context.MODE_PRIVATE)
        val savedLang = prefs.getString("language", null)

        if (savedLang != null && savedLang != "auto") {
            api.setLocale(savedLang)
        } else {
            // Detect Android system language (e.g., en-US -> en_US)
            api.setLocale(java.util.Locale.getDefault().toLanguageTag().replace("-", "_"))
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val name = "Task Reminders"
            val importance = NotificationManager.IMPORTANCE_HIGH
            val channel = NotificationChannel("CFAIT_ALARMS", name, importance).apply {
                description = "Notifications for task reminders and alarms"
            }
            val notificationManager = getSystemService(NotificationManager::class.java)
            notificationManager?.createNotificationChannel(channel)
        }
    }
}
