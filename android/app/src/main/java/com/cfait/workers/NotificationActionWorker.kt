// File: android/app/src/main/java/com/cfait/workers/NotificationActionWorker.kt
package com.cfait.workers

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.cfait.CfaitApplication
import com.cfait.util.AlarmScheduler

class NotificationActionWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val KEY_ACTION = "action"
        const val KEY_TASK_UID = "task_uid"
        const val KEY_ALARM_UID = "alarm_uid"
        const val ACTION_SNOOZE = "SNOOZE"
        const val ACTION_DISMISS = "DISMISS"
        const val BROADCAST_REFRESH = "com.cfait.REFRESH_UI"
    }

    override suspend fun doWork(): Result {
        return try {
            val action = inputData.getString(KEY_ACTION)
            val taskUid = inputData.getString(KEY_TASK_UID)
            val alarmUid = inputData.getString(KEY_ALARM_UID)

            if (action == null || taskUid == null || alarmUid == null) {
                Log.e("CfaitNotificationAction", "Missing required parameters")
                return Result.failure()
            }

            Log.d("CfaitNotificationAction", "Processing action: $action for task: $taskUid")

            // FIX: Use the singleton API instance from Application
            val app = context.applicationContext as CfaitApplication
            val api = app.api

            // Note: We do NOT call api.loadFromCache() here because the singleton
            // is already initialized in Application.onCreate().

            when (action) {
                ACTION_SNOOZE -> {
                    api.snoozeAlarm(taskUid, alarmUid, 15u)
                    Log.d("CfaitNotificationAction", "Alarm snoozed")
                }

                ACTION_DISMISS -> {
                    api.dismissAlarm(taskUid, alarmUid)
                    Log.d("CfaitNotificationAction", "Alarm dismissed")
                }

                else -> return Result.failure()
            }

            // Reschedule next alarm
            AlarmScheduler.scheduleNextAlarm(context, api)

            // FIX: Notify the UI (if active) that data has changed
            val intent = Intent(BROADCAST_REFRESH)
            intent.setPackage(context.packageName) // Restrict to own app
            context.sendBroadcast(intent)

            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitNotificationAction", "Error", e)
            Result.retry()
        }
    }
}
