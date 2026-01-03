// Android BroadcastReceiver for handling system alarm events.
package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import com.cfait.workers.AlarmWorker

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Log.d("CfaitAlarmReceiver", "Alarm triggered, delegating to WorkManager")

        // Create a work request for alarm processing
        val workRequest = OneTimeWorkRequestBuilder<AlarmWorker>()
            .build()

        // Enqueue the work
        WorkManager.getInstance(context).enqueueUniqueWork(
            "cfait_alarm_processing",
            ExistingWorkPolicy.REPLACE,
            workRequest
        )

        Log.d("CfaitAlarmReceiver", "Work enqueued with ID: ${workRequest.id}")
    }
}
