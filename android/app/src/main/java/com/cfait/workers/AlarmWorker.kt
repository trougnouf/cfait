package com.cfait.workers

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.cfait.MainActivity
import com.cfait.R
import com.cfait.core.CfaitMobile
import com.cfait.receivers.NotificationActionReceiver
import com.cfait.util.AlarmScheduler

/**
 * WorkManager-based worker for processing alarms.
 *
 * This approach provides several advantages over direct BroadcastReceiver execution:
 * 1. Automatic WakeLock management - guarantees the app stays awake during execution
 * 2. No 10-second BroadcastReceiver time limit
 * 3. Better handling of process death scenarios
 * 4. Expedited execution for time-sensitive work
 */
class AlarmWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitAlarmWorker", "Starting alarm processing")

            // Initialize the Rust backend
            // This may take some time on cold boot, but WorkManager ensures we have time
            val api = CfaitMobile(context.filesDir.absolutePath)

            // Load task data from cache
            // This reads and deserializes journal.json
            api.loadFromCache()

            // Get all alarms that should fire now
            val firing = api.getFiringAlarms()

            if (firing.isNotEmpty()) {
                Log.d("CfaitAlarmWorker", "Found ${firing.size} firing alarm(s)")

                // Show a notification for each firing alarm
                // Note: Notification channel is created once at app startup in CfaitApplication
                firing.forEach { alarm ->
                    showNotification(
                        context = context,
                        title = alarm.title,
                        body = alarm.body,
                        taskUid = alarm.taskUid,
                        alarmUid = alarm.alarmUid
                    )
                }
            } else {
                Log.d("CfaitAlarmWorker", "No alarms to fire")
            }

            // Schedule the next alarm
            AlarmScheduler.scheduleNextAlarm(context, api)

            Log.d("CfaitAlarmWorker", "Alarm processing completed successfully")
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitAlarmWorker", "Error processing alarms", e)
            // Retry on failure to ensure alarms aren't missed
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
        // Check notification permission
        if (ActivityCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            Log.w("CfaitAlarmWorker", "Missing POST_NOTIFICATIONS permission")
            return
        }

        // Intent to open the app when notification is tapped
        val tapIntent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val tapPending = PendingIntent.getActivity(
            context,
            taskUid.hashCode(),
            tapIntent,
            PendingIntent.FLAG_IMMUTABLE
        )

        // Snooze action
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

        // Dismiss action
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

        // Build the notification
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

        // Show the notification with a unique ID per alarm
        NotificationManagerCompat.from(context).notify(alarmUid.hashCode(), notification)
    }
}
