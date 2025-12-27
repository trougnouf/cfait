package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import com.cfait.workers.NotificationActionWorker

/**
 * BroadcastReceiver that handles notification action clicks (Snooze/Dismiss).
 *
 * This receiver immediately:
 * 1. Cancels the notification to provide instant user feedback
 * 2. Delegates the actual work to WorkManager for reliable execution
 *
 * This ensures the alarm state is properly updated even if the app process
 * is killed or under memory pressure.
 */
class NotificationActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val taskUid = intent.getStringExtra("T_UID")
        val alarmUid = intent.getStringExtra("A_UID")
        val action = intent.action

        if (taskUid == null || alarmUid == null || action == null) {
            Log.e("CfaitNotificationAction", "Missing required intent extras or action")
            return
        }

        Log.d("CfaitNotificationAction", "Received action: $action for alarm: $alarmUid")

        // Immediately dismiss the notification to provide instant user feedback
        NotificationManagerCompat.from(context).cancel(alarmUid.hashCode())

        // Prepare input data for the worker
        val inputData = Data.Builder()
            .putString(NotificationActionWorker.KEY_ACTION, action)
            .putString(NotificationActionWorker.KEY_TASK_UID, taskUid)
            .putString(NotificationActionWorker.KEY_ALARM_UID, alarmUid)
            .build()

        // Create a work request with the action data
        val workRequest = OneTimeWorkRequestBuilder<NotificationActionWorker>()
            .setInputData(inputData)
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .build()

        // Enqueue the work
        // Use unique work name to prevent duplicate processing
        WorkManager.getInstance(context).enqueueUniqueWork(
            "cfait_notification_action_${alarmUid}",
            ExistingWorkPolicy.REPLACE,
            workRequest
        )

        Log.d("CfaitNotificationAction", "Work enqueued with ID: ${workRequest.id}")
    }
}
