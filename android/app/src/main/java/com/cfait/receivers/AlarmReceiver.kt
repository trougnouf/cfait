package com.cfait.receivers

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.cfait.MainActivity
import com.cfait.R
import com.cfait.core.CfaitMobile
import com.cfait.util.AlarmScheduler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        // We must initialize the API here because the app might be dead
        // Use the generic files dir
        val api = CfaitMobile(context.filesDir.absolutePath)

        // Go async
        CoroutineScope(Dispatchers.IO).launch {
            // FIX: Load data from disk! The store is empty on cold boot otherwise.
            api.loadFromCache()

            // 1. Get Firing Alarms
            val firing = api.getFiringAlarms()

            if (firing.isNotEmpty()) {
                createChannel(context)

                firing.forEach { alarm ->
                    showNotification(context, alarm.title, alarm.body, alarm.taskUid, alarm.alarmUid)
                }
            }

            // 2. Schedule Next (The loop continues)
            AlarmScheduler.scheduleNextAlarm(context, api)
        }
    }

    private fun showNotification(context: Context, title: String, body: String, tUid: String, aUid: String) {
        if (ActivityCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return
        }

        // Tap Intent -> Open App
        val tapIntent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val tapPending = PendingIntent.getActivity(context, tUid.hashCode(), tapIntent, PendingIntent.FLAG_IMMUTABLE)

        // Action: Snooze
        val snoozeIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = "SNOOZE"
            putExtra("T_UID", tUid)
            putExtra("A_UID", aUid)
        }
        val snoozePending = PendingIntent.getBroadcast(
            context,
            (tUid + "S").hashCode(),
            snoozeIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Action: Dismiss
        val dismissIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = "DISMISS"
            putExtra("T_UID", tUid)
            putExtra("A_UID", aUid)
        }
        val dismissPending = PendingIntent.getBroadcast(
            context,
            (tUid + "D").hashCode(),
            dismissIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notif = NotificationCompat.Builder(context, "CFAIT_ALARMS")
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(title)
            .setContentText(body)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(tapPending)
            .setAutoCancel(true)
            .addAction(R.drawable.ic_launcher_foreground, "Snooze (15m)", snoozePending)
            .addAction(R.drawable.ic_launcher_foreground, "Dismiss", dismissPending)
            .build()

        // Use distinct ID per alarm/task combination
        NotificationManagerCompat.from(context).notify(aUid.hashCode(), notif)
    }

    private fun createChannel(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val name = "Task Reminders"
            val importance = NotificationManager.IMPORTANCE_HIGH
            val channel = NotificationChannel("CFAIT_ALARMS", name, importance)
            val nm = context.getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(channel)
        }
    }
}
