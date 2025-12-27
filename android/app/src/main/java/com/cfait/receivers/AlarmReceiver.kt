package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import com.cfait.workers.AlarmWorker

/**
 * BroadcastReceiver that handles alarm triggers from AlarmManager.
 *
 * This receiver immediately delegates work to WorkManager instead of executing
 * directly. This ensures:
 * - Work continues even if the receiver's 10-second limit expires
 * - WakeLock is automatically managed by WorkManager
 * - Better reliability when the app process is dead or device is in Doze mode
 */
class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Log.d("CfaitAlarmReceiver", "Alarm triggered, delegating to WorkManager")

        // Create a work request for alarm processing
        // Use setExpedited() to request high-priority execution with WakeLock
        val workRequest = OneTimeWorkRequestBuilder<AlarmWorker>()
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .build()

        // Enqueue the work
        // Use enqueueUniqueWork to prevent duplicate processing if multiple alarms fire quickly
        WorkManager.getInstance(context).enqueueUniqueWork(
            "cfait_alarm_processing",
            ExistingWorkPolicy.REPLACE,
            workRequest
        )

        Log.d("CfaitAlarmReceiver", "Work enqueued with ID: ${workRequest.id}")
    }
}
