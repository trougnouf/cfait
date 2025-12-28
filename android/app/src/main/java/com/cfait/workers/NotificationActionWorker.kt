package com.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.cfait.core.CfaitMobile
import com.cfait.util.AlarmScheduler

/**
 * WorkManager-based worker for handling notification actions (snooze/dismiss).
 *
 * This ensures that actions triggered from notifications are processed reliably,
 * even if the app process is dead or under memory pressure.
 */
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

            Log.d("CfaitNotificationAction", "Processing action: $action for task: $taskUid, alarm: $alarmUid")

            // Initialize the Rust backend
            val api = CfaitMobile(context.filesDir.absolutePath)
            api.loadFromCache()

            Log.d("CfaitNotificationAction", "Before action: next alarm timestamp = ${api.getNextAlarmTimestamp()}")

            // Process the action
            when (action) {
                ACTION_SNOOZE -> {
                    // Snooze for 15 minutes
                    Log.d("CfaitNotificationAction", "Calling snoozeAlarm...")
                    api.snoozeAlarm(taskUid, alarmUid, 15u)
                    Log.d("CfaitNotificationAction", "Alarm snoozed for 15 minutes")
                }

                ACTION_DISMISS -> {
                    Log.d("CfaitNotificationAction", "Calling dismissAlarm...")
                    api.dismissAlarm(taskUid, alarmUid)
                    Log.d("CfaitNotificationAction", "Alarm dismissed")
                }

                else -> {
                    Log.w("CfaitNotificationAction", "Unknown action: $action")
                    return Result.failure()
                }
            }

            Log.d("CfaitNotificationAction", "After action: next alarm timestamp = ${api.getNextAlarmTimestamp()}")

            // Reschedule alarms since we've modified the alarm state
            AlarmScheduler.scheduleNextAlarm(context, api)

            Log.d("CfaitNotificationAction", "Action completed successfully")
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitNotificationAction", "Error processing notification action", e)
            // Retry to ensure the action isn't lost
            Result.retry()
        }
    }
}
