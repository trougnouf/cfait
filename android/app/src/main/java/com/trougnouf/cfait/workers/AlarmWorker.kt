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
import androidx.core.app.RemoteInput
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
                        config.snoozeShort
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
        shortMins: UInt
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

        // 1. Snooze Default (Short)
        val snoozeDefaultIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_SNOOZE_DEFAULT
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val snoozeDefaultPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "SD").hashCode(),
            snoozeDefaultIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // 2. Snooze Custom (Inline Reply)
        val snoozeCustomKey = "snooze_custom_duration"
        val remoteInput = RemoteInput.Builder(snoozeCustomKey)
            .setLabel("Duration (e.g. 30m, 1h)")
            .build()

        val snoozeCustomIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_SNOOZE_CUSTOM
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val snoozeCustomPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "SC").hashCode(),
            snoozeCustomIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE // Mutable for RemoteInput
        )

        val customAction = NotificationCompat.Action.Builder(
            R.drawable.ic_launcher_foreground,
            "Snooze...",
            snoozeCustomPending
        ).addRemoteInput(remoteInput).build()

        // 3. Done Action
        val doneIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_DONE
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val donePending = PendingIntent.getBroadcast(
            context,
            (taskUid + "DONE").hashCode(),
            doneIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // 4. Cancel Action
        val cancelIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_CANCEL
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val cancelPending = PendingIntent.getBroadcast(
            context,
            (taskUid + "CANCEL").hashCode(),
            cancelIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Dismiss action (swiping away)
        val deleteIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            action = NotificationActionWorker.ACTION_DISMISS
            putExtra("T_UID", taskUid)
            putExtra("A_UID", alarmUid)
        }
        val deletePending = PendingIntent.getBroadcast(
            context,
            (taskUid + "DEL").hashCode(),
            deleteIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(context, "CFAIT_ALARMS")
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(title)
            .setContentText(body)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setContentIntent(tapPending)
            .setDeleteIntent(deletePending) // Swipe = Dismiss
            .setAutoCancel(true)
            .addAction(R.drawable.ic_launcher_foreground, "Snooze (${formatMins(shortMins)})", snoozeDefaultPending)
            .addAction(customAction)
            .addAction(R.drawable.ic_launcher_foreground, "Done", donePending)
            .addAction(R.drawable.ic_launcher_foreground, "Cancel", cancelPending)
            .build()

        NotificationManagerCompat.from(context).notify(alarmUid.hashCode(), notification)
    }
}
