// SPDX-License-Identifier: GPL-3.0-or-later
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
import kotlinx.coroutines.CancellationException

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
        const val ACTION_DISMISS_ONGOING = "DISMISS_ONGOING"

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
                        com.trougnouf.cfait.util.NotificationHelper.showActiveTaskNotification(context, task, alarmUid)
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

                ACTION_DISMISS_ONGOING -> {
                    // Record that user deliberately swiped away this specific ongoing notification
                    val prefs = context.getSharedPreferences("cfait_ongoing_notifs", Context.MODE_PRIVATE)
                    prefs.edit().putBoolean(taskUid, true).apply()
                }

                else -> {
                    Log.w("CfaitNotificationAction", "Unknown action: $action")
                    return Result.failure()
                }
            }

            // Refresh UI and Scheduler
            AlarmScheduler.scheduleNextAlarm(context, api)
            AlarmScheduler.cleanupObsoleteNotifications(
                context,
                api
            ) // <- Add cleanup pass to prune stale notifications
            val intent = Intent(BROADCAST_REFRESH)
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)

            Result.success()
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.e("CfaitNotificationAction", "Error", e)
            Result.retry()
        }
    }

}
