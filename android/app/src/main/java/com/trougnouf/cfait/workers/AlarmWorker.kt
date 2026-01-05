// Background worker for processing firing alarms.
package com.trougnouf.cfait.workers

import android.Manifest
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.R
import com.trougnouf.cfait.receivers.NotificationActionReceiver
import com.trougnouf.cfait.util.AlarmScheduler

class AlarmWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitAlarmWorker", "Starting alarm processing")

            val app = context.applicationContext as CfaitApplication
            val api = app.api

            // Need config to get snooze durations for button labels
            val config = api.getConfig()

            val firing = api.getFiringAlarms()

            if (firing.isNotEmpty()) {
                Log.d("CfaitAlarmWorker", "Found ${firing.size} firing alarm(s)")
                firing.forEach { alarm ->
                    showNotification(
                        context,
                        alarm.title,
                        alarm.body,
                        alarm.taskUid,
                        alarm.alarmUid,
                        config.snoozeShort,
                        config.snoozeLong
                    )
                }
            }

            AlarmScheduler.scheduleNextAlarm(context, api)
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitAlarmWorker", "Error", e)
            Result.retry()
        }
    }

    private fun formatMins(mins: UInt): String {
        val m = mins.toInt()
        return if (m >= 60) {
            val h = m / 60
            // Handle 1h, 1.5h, etc if you want, but simple int hours is usually enough for buttons
            if (m % 60 == 0) "${h}h" else "${h}h ${m % 60}m"
        } else {
            "${m}m"
        }
    }

    private fun showNotification(
        context: Context,
        title: String,
        body: String,
        taskUid: String,
        alarmUid: String,
        shortMins: UInt,
        longMins: UInt
    ) {
        if (ActivityCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return
        }

        val tapIntent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val tapPending = PendingIntent.getActivity(
            context,
            taskUid.hashCode(),
            tapIntent,
            PendingIntent.FLAG_IMMUTABLE
        )

        // 1. Snooze Short Action
        val snoozeShortIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_SNOOZE_SHORT
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        // Unique RequestCode: hash + "SS"
        val snoozeShortPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "SS").hashCode(),
            snoozeShortIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // 2. Snooze Long Action
        val snoozeLongIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_SNOOZE_LONG
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        // Unique RequestCode: hash + "SL"
        val snoozeLongPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "SL").hashCode(),
            snoozeLongIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // 3. Dismiss Action
        val dismissIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_DISMISS
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val dismissPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "D").hashCode(),
            dismissIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(context, "CFAIT_ALARMS")
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(title)
            .setContentText(body)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(tapPending)
            .setAutoCancel(true)
            // Add 3 actions
            .addAction(R.drawable.ic_launcher_foreground, "Snooze (${formatMins(shortMins)})", snoozeShortPending)
            .addAction(R.drawable.ic_launcher_foreground, "Snooze (${formatMins(longMins)})", snoozeLongPending)
            .addAction(R.drawable.ic_launcher_foreground, "Dismiss", dismissPending)
            .build()

        NotificationManagerCompat.from(context).notify(alarmUid.hashCode(), notification)
    }
}
