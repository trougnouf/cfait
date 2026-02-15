// Worker handling background notification interactions.
package com.trougnouf.cfait.workers

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.R
import com.trougnouf.cfait.receivers.NotificationActionReceiver
import com.trougnouf.cfait.util.AlarmScheduler

class NotificationActionWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val KEY_ACTION = "action"
        const val KEY_TASK_UID = "task_uid"
        const val KEY_ALARM_UID = "alarm_uid"
        const val KEY_CUSTOM_INPUT = "custom_input"

        // Actions
        const val ACTION_SNOOZE_CUSTOM = "SNOOZE_CUSTOM"
        const val ACTION_START = "START"
        const val ACTION_PAUSE = "PAUSE"
        const val ACTION_DONE = "DONE"
        const val ACTION_CANCEL = "CANCEL"
        const val ACTION_DISMISS = "DISMISS"

        const val BROADCAST_REFRESH = "com.trougnouf.cfait.REFRESH_UI"
        const val CHANNEL_ALARMS = "CFAIT_ALARMS"
    }

    override suspend fun doWork(): Result {
        return try {
            val action = inputData.getString(KEY_ACTION)
            val taskUid = inputData.getString(KEY_TASK_UID)
            val alarmUid = inputData.getString(KEY_ALARM_UID)
            val customInput = inputData.getString(KEY_CUSTOM_INPUT)

            if (action == null || taskUid == null || alarmUid == null) {
                Log.e("CfaitNotificationAction", "Missing required parameters")
                return Result.failure()
            }

            Log.d("CfaitNotificationAction", "Processing action: $action for task: $taskUid")

            val app = context.applicationContext as CfaitApplication
            val api = app.api

            when (action) {
                ACTION_SNOOZE_CUSTOM -> {
                    val input = customInput ?: "10m"
                    val mins = api.parseDurationString(input) ?: 10u
                    api.snoozeAlarm(taskUid, alarmUid, mins)
                    Log.d("CfaitNotificationAction", "Alarm custom snoozed for $mins minutes")
                }

                ACTION_START -> {
                    // 1. Update state in backend to start tracking
                    api.startTask(taskUid)
                    // 2. Dismiss the specific alarm that triggered this
                    api.dismissAlarm(taskUid, alarmUid)

                    // 3. Fetch fresh task data to get accurate time tracking info
                    val task = api.getTaskByUid(taskUid)
                    if (task != null) {
                        showActiveTaskNotification(context, task, alarmUid)
                    }
                }

                ACTION_PAUSE -> {
                    api.pauseTask(taskUid)
                    // Cancel any active persistent notification for this task
                    val notificationManager =
                        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                    notificationManager.cancel(taskUid.hashCode())
                }

                ACTION_DONE -> {
                    api.toggleTask(taskUid)
                    val notificationManager =
                        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                    notificationManager.cancel(taskUid.hashCode())
                }

                ACTION_CANCEL -> {
                    api.setStatusCancelled(taskUid)
                    val notificationManager =
                        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                    notificationManager.cancel(taskUid.hashCode())
                }

                ACTION_DISMISS -> {
                    api.dismissAlarm(taskUid, alarmUid)
                }

                else -> {
                    Log.w("CfaitNotificationAction", "Unknown action: $action")
                    return Result.failure()
                }
            }

            // Refresh UI and Scheduler
            AlarmScheduler.scheduleNextAlarm(context, api)
            val intent = Intent(BROADCAST_REFRESH)
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)

            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitNotificationAction", "Error", e)
            Result.retry()
        }
    }

    private fun showActiveTaskNotification(
        context: Context,
        task: com.trougnouf.cfait.core.MobileTask,
        originalAlarmUid: String
    ) {
        val notificationId = task.uid.hashCode()

        // Calculate base time for Chronometer
        val now = System.currentTimeMillis()
        val startTs = task.lastStartedAt ?: (now / 1000)
        val currentSessionMs = (now - (startTs * 1000))
        val totalSpentMs = (task.timeSpentSeconds.toLong() * 1000) + currentSessionMs

        val chronoBase = SystemClock.elapsedRealtime() - totalSpentMs

        // Pause Action
        val pauseIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            this.action = ACTION_PAUSE
            putExtra("T_UID", task.uid)
            putExtra("A_UID", originalAlarmUid)
        }
        val pausePending = PendingIntent.getBroadcast(
            context,
            (task.uid + "PAUSE").hashCode(),
            pauseIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Done Action
        val doneIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            this.action = ACTION_DONE
            putExtra("T_UID", task.uid)
            putExtra("A_UID", originalAlarmUid)
        }
        val donePending = PendingIntent.getBroadcast(
            context,
            (task.uid + "DONE_ACT").hashCode(),
            doneIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Tap opens app
        val tapIntent = Intent(context, MainActivity::class.java)
        val tapPending =
            PendingIntent.getActivity(context, task.uid.hashCode(), tapIntent, PendingIntent.FLAG_IMMUTABLE)

        val notification = NotificationCompat.Builder(context, CHANNEL_ALARMS)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle("In Progress: ${task.summary}")
            .setUsesChronometer(true)
            .setWhen(System.currentTimeMillis() - totalSpentMs)
            .setShowWhen(true)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setDeleteIntent(pausePending)
            .setContentIntent(tapPending)
            .addAction(R.drawable.ic_launcher_foreground, "Pause", pausePending)
            .addAction(R.drawable.ic_launcher_foreground, "Done", donePending)
            .build()

        val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(notificationId, notification)
    }
}
