// SPDX-License-Identifier: GPL-3.0-or-later
// Android Receiver for handling notification actions (Snooze/Dismiss).
package com.trougnouf.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import com.trougnouf.cfait.workers.NotificationActionWorker

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
        val taskUid = intent.getStringExtra(NotificationActionWorker.EXTRA_TASK_UID)
        val alarmUid = intent.getStringExtra(NotificationActionWorker.EXTRA_ALARM_UID)
        val action = intent.action

        if (taskUid == null || alarmUid == null || action == null) {
            Log.e("CfaitNotificationAction", "Missing required intent extras or action")
            return
        }

        Log.d("CfaitNotificationAction", "Received action: $action for alarm: $alarmUid")

        // Immediately dismiss the relevant notification(s) to provide instant
        // user feedback. Use a try-catch in case the permission was revoked.
        try {
            val manager = NotificationManagerCompat.from(context)
            when (action) {
                // Completing or starting the task makes both notifications stale
                NotificationActionWorker.ACTION_DONE,
                NotificationActionWorker.ACTION_START -> {
                    manager.cancel((taskUid + "_alarm").hashCode())
                    manager.cancel(taskUid.hashCode())
                }
                // Actions on the ongoing "in progress" notification
                NotificationActionWorker.ACTION_PAUSE,
                NotificationActionWorker.ACTION_DISMISS_ONGOING -> {
                    manager.cancel(taskUid.hashCode())
                }
                // Snooze and alarm dismiss leave the ongoing timer untouched
                else -> {
                    manager.cancel((taskUid + "_alarm").hashCode())
                }
            }
        } catch (e: SecurityException) {
            Log.w("CfaitNotificationAction", "Could not cancel notification due to SecurityException", e)
        }

        // Check for RemoteInput (Custom Snooze)
        val remoteInput = RemoteInput.getResultsFromIntent(intent)
        val customInput = remoteInput?.getCharSequence(NotificationActionWorker.EXTRA_SNOOZE_INPUT)?.toString()

        // Prepare input data for the worker, passing along the specific action.
        val dataBuilder = Data.Builder()
            .putString(NotificationActionWorker.KEY_ACTION, action)
            .putString(NotificationActionWorker.KEY_TASK_UID, taskUid)
            .putString(NotificationActionWorker.KEY_ALARM_UID, alarmUid)

        if (customInput != null) {
            dataBuilder.putString(NotificationActionWorker.KEY_CUSTOM_INPUT, customInput)
        }

        val inputData = dataBuilder.build()

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
