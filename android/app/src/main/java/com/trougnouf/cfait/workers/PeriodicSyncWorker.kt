package com.trougnouf.cfait.workers

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.AlarmScheduler

/**
 * Periodic background worker that performs a full sync and refreshes alarms/UI.
 *
 * This worker is intended to be scheduled via WorkManager (PeriodicWorkRequest).
 * It will call the mobile API's `sync()` method, reschedule the next alarm check,
 * and broadcast a UI refresh intent so any active UI can react.
 */
class PeriodicSyncWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitPeriodicSync", "Running background sync")

            val app = context.applicationContext as CfaitApplication
            val api = app.api

            // Perform sync with backend / local store
            api.sync()

            // Ensure alarms are up-to-date after sync
            AlarmScheduler.scheduleNextAlarm(context, api)
            AlarmScheduler.cleanupObsoleteNotifications(context, api)

            // Notify UI to refresh if open
            val intent = Intent("com.trougnouf.cfait.REFRESH_UI")
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)

            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitPeriodicSync", "Sync failed", e)
            Result.retry()
        }
    }
}
