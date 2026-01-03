// Android Application class initializing the Rust backend.
package com.cfait

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.os.Build
import com.cfait.core.CfaitMobile

class CfaitApplication : Application() {
    lateinit var api: CfaitMobile
        private set

    override fun onCreate() {
        super.onCreate()

        // Create notification channel once at app startup (Android O+)
        // This avoids redundant IPC overhead on every alarm fire
        createNotificationChannel()

        // Initialize the Rust backend once for the lifetime of the application
        api = CfaitMobile(filesDir.absolutePath)

        // Preload data into memory immediately so UI is ready faster
        api.loadFromCache()
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
