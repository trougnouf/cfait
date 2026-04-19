// SPDX-License-Identifier: GPL-3.0-or-later
// Background worker for rescheduling alarms on boot.
package com.trougnouf.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.AlarmScheduler
import com.trougnouf.cfait.util.NotificationHelper
import kotlinx.coroutines.CancellationException

class BootWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitBootWorker", "Rescheduling alarms after boot")

            // Use Singleton
            val app = context.applicationContext as CfaitApplication
            val api = app.api

            AlarmScheduler.scheduleNextAlarm(context, api)
            NotificationHelper.updateOngoingNotifications(context, api)
            Result.success()
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.e("CfaitBootWorker", "Error", e)
            Result.retry()
        }
    }
}
