// Worker handling background notification interactions.
package com.trougnouf.cfait.workers

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.AlarmScheduler

class NotificationActionWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val KEY_ACTION = "action"
        const val KEY_TASK_UID = "task_uid"
        const val KEY_ALARM_UID = "alarm_uid"

        // Define distinct actions
        const val ACTION_SNOOZE_SHORT = "SNOOZE_SHORT"
        const val ACTION_SNOOZE_LONG = "SNOOZE_LONG"
        const val ACTION_DISMISS = "DISMISS"

        const val BROADCAST_REFRESH = "com.trougnouf.cfait.REFRESH_UI"
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

            val app = context.applicationContext as CfaitApplication
            val api = app.api

            // Fetch config to get the actual minutes for short/long snooze
            val config = api.getConfig()

            when (action) {
                ACTION_SNOOZE_SHORT -> {
                    val mins = config.snoozeShort
                    api.snoozeAlarm(taskUid, alarmUid, mins)
                    Log.d("CfaitNotificationAction", "Alarm snoozed for $mins minutes (Short)")
                }

                ACTION_SNOOZE_LONG -> {
                    val mins = config.snoozeLong
                    api.snoozeAlarm(taskUid, alarmUid, mins)
                    Log.d("CfaitNotificationAction", "Alarm snoozed for $mins minutes (Long)")
                }

                ACTION_DISMISS -> {
                    api.dismissAlarm(taskUid, alarmUid)
                    Log.d("CfaitNotificationAction", "Alarm dismissed")
                }

                else -> {
                    Log.w("CfaitNotificationAction", "Unknown action: $action")
                    return Result.failure()
                }
            }

            // Reschedule next alarm
            AlarmScheduler.scheduleNextAlarm(context, api)

            // Notify UI
            val intent = Intent(BROADCAST_REFRESH)
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)

            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitNotificationAction", "Error", e)
            Result.retry()
        }
    }
}
