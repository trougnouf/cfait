// Android Receiver for handling notification actions (Snooze/Dismiss).
package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import com.cfait.workers.NotificationActionWorker

/**
 * BroadcastReceiver that handles notification action clicks (Snooze/Dismiss).
 *
 * This receiver immediately:
 * 1. Cancels the notification from the system tray to provide instant user feedback.
 * 2. Delegates the actual work (calling the Rust backend) to WorkManager for reliable
 *    background execution, passing the specific action to be performed.
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

        // Immediately dismiss the notification to provide instant user feedback.
        // Use a try-catch in case the notification permission was revoked.
        try {
            NotificationManagerCompat.from(context).cancel(alarmUid.hashCode())
        } catch (e: SecurityException) {
            Log.w("CfaitNotificationAction", "Could not cancel notification due to SecurityException", e)
        }

        // Prepare input data for the worker, passing along the specific action.
        val inputData = Data.Builder()
            .putString(NotificationActionWorker.KEY_ACTION, action)
            .putString(NotificationActionWorker.KEY_TASK_UID, taskUid)
            .putString(NotificationActionWorker.KEY_ALARM_UID, alarmUid)
            .build()

        // Create a work request for the NotificationActionWorker.
        // We do not use .setExpedited() here to avoid potential crashes on Android 12+
        // if the app is in the background, as it would require foreground service permissions.
        val workRequest = OneTimeWorkRequestBuilder<NotificationActionWorker>()
            .setInputData(inputData)
            .build()

        // Enqueue the work with a unique name based on the alarm to prevent duplicates.
        // REPLACE ensures that if the user clicks another action on the same notification
        // while the first is pending, the new action takes precedence.
        WorkManager.getInstance(context).enqueueUniqueWork(
            "cfait_notification_action_${alarmUid}",
            ExistingWorkPolicy.REPLACE,
            workRequest
        )

        Log.d("CfaitNotificationAction", "Work enqueued with ID: ${workRequest.id}")
    }
}
