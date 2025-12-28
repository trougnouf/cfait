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
        try {
            NotificationManagerCompat.from(context).cancel(alarmUid.hashCode())
        } catch (e: SecurityException) {
            // Can happen on some devices if permission revoked
        }

        // Prepare input data for the worker
        val inputData = Data.Builder()
            .putString(NotificationActionWorker.KEY_ACTION, action)
            .putString(NotificationActionWorker.KEY_TASK_UID, taskUid)
            .putString(NotificationActionWorker.KEY_ALARM_UID, alarmUid)
            .build()

        // Create a work request with the action data
        // FIX: Removed setExpedited() to prevent crashes on Android 12+ when
        // getForegroundInfo() is not implemented. Standard priority is sufficient here.
        val workRequest = OneTimeWorkRequestBuilder<NotificationActionWorker>()
            .setInputData(inputData)
            .build()

        // Enqueue the work
        WorkManager.getInstance(context).enqueueUniqueWork(
            "cfait_notification_action_${alarmUid}",
            ExistingWorkPolicy.REPLACE,
            workRequest
        )

        Log.d("CfaitNotificationAction", "Work enqueued with ID: ${workRequest.id}")
    }
}
