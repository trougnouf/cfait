// Android Receiver for rescheduling alarms after device boot.
package com.cfait.receivers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import com.cfait.workers.BootWorker

/**
 * BroadcastReceiver that handles device boot completion and app updates.
 *
 * This receiver immediately delegates work to WorkManager instead of executing
 * directly. This ensures:
 * - Work continues even if the receiver's 10-second limit expires
 * - Better reliability during boot when the device may be under load
 * - Alarms are properly rescheduled even if the boot process is slow
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED,
            Intent.ACTION_MY_PACKAGE_REPLACED -> {
                Log.d("CfaitBootReceiver", "Boot/update detected, delegating to WorkManager")

                // Create a work request for alarm rescheduling
                val workRequest = OneTimeWorkRequestBuilder<BootWorker>()
                    .build()

                // Enqueue the work
                // Use enqueueUniqueWork to prevent duplicate rescheduling
                WorkManager.getInstance(context).enqueueUniqueWork(
                    "cfait_boot_reschedule",
                    ExistingWorkPolicy.REPLACE,
                    workRequest
                )

                Log.d("CfaitBootReceiver", "Boot work enqueued with ID: ${workRequest.id}")
            }
        }
    }
}
