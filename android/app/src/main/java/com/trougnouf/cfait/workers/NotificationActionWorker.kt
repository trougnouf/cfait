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
         const val KEY_CUSTOM_INPUT = "custom_input"

         const val ACTION_SNOOZE_DEFAULT = "SNOOZE_DEFAULT"
         const val ACTION_SNOOZE_CUSTOM = "SNOOZE_CUSTOM"
         const val ACTION_DONE = "DONE"
         const val ACTION_CANCEL = "CANCEL"
         const val ACTION_DISMISS = "DISMISS"

         const val BROADCAST_REFRESH = "com.trougnouf.cfait.REFRESH_UI"
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
             val config = api.getConfig()

             when (action) {
                 ACTION_SNOOZE_DEFAULT -> {
                     val mins = config.snoozeShort
                     api.snoozeAlarm(taskUid, alarmUid, mins)
                     Log.d("CfaitNotificationAction", "Alarm snoozed for $mins minutes")
                 }

                 ACTION_SNOOZE_CUSTOM -> {
                     val input = customInput ?: "10m"
                     // Parse using backend parser wrapper
                     val mins = api.parseDurationString(input) ?: 10u
                     api.snoozeAlarm(taskUid, alarmUid, mins)
                     Log.d("CfaitNotificationAction", "Alarm custom snoozed for $mins minutes")
                 }

                 ACTION_DONE -> {
                     // Completing the task effectively handles the alarm logic via recycle/sync
                     api.toggleTask(taskUid)
                     Log.d("CfaitNotificationAction", "Task marked done from alarm")
                 }

                 ACTION_CANCEL -> {
                     api.setStatusCancelled(taskUid)
                     Log.d("CfaitNotificationAction", "Task cancelled from alarm")
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
 }
