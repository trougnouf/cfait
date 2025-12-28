// File: android/app/src/main/java/com/cfait/workers/AlarmWorker.kt
package com.cfait.workers

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
import com.cfait.CfaitApplication
import com.cfait.MainActivity
import com.cfait.R
import com.cfait.receivers.NotificationActionReceiver
import com.cfait.util.AlarmScheduler

class AlarmWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitAlarmWorker", "Starting alarm processing")

            // FIX: Use Singleton API
            val app = context.applicationContext as CfaitApplication
            val api = app.api

            val firing = api.getFiringAlarms()

            if (firing.isNotEmpty()) {
                Log.d("CfaitAlarmWorker", "Found ${firing.size} firing alarm(s)")
                firing.forEach { alarm ->
                    showNotification(context, alarm.title, alarm.body, alarm.taskUid, alarm.alarmUid)
                }
            }

            AlarmScheduler.scheduleNextAlarm(context, api)
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitAlarmWorker", "Error", e)
            Result.retry()
        }
    }

    private fun showNotification(
        context: Context,
        title: String,
        body: String,
        taskUid: String,
        alarmUid: String
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

        val snoozeIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = "SNOOZE"
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val snoozePending = PendingIntent.getBroadcast(
            context,
            (taskUid + "S").hashCode(),
            snoozeIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val dismissIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = "DISMISS"
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
            .addAction(R.drawable.ic_launcher_foreground, "Snooze (15m)", snoozePending)
            .addAction(R.drawable.ic_launcher_foreground, "Dismiss", dismissPending)
            .build()

        NotificationManagerCompat.from(context).notify(alarmUid.hashCode(), notification)
    }
}
