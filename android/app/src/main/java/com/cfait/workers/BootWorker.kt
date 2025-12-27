package com.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.cfait.core.CfaitMobile
import com.cfait.util.AlarmScheduler

/**
 * WorkManager-based worker for rescheduling alarms after device boot.
 *
 * This ensures that alarms are properly restored after device reboot,
 * even if the boot process is slow or the app is under memory pressure.
 * WorkManager provides better reliability than executing directly in a
 * BroadcastReceiver, which has a 10-second time limit.
 */
class BootWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitBootWorker", "Starting alarm rescheduling after boot")

            // Initialize the Rust backend
            val api = CfaitMobile(context.filesDir.absolutePath)

            // Load task data from cache
            api.loadFromCache()

            // Reschedule the next alarm
            AlarmScheduler.scheduleNextAlarm(context, api)

            Log.d("CfaitBootWorker", "Boot alarm rescheduling completed successfully")
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitBootWorker", "Error rescheduling alarms after boot", e)
            // Retry to ensure alarms aren't lost after reboot
            Result.retry()
        }
    }
}
