// Background worker for rescheduling alarms on boot.
package com.trougnouf.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.AlarmScheduler

class BootWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitBootWorker", "Rescheduling alarms after boot")

            // FIX: Use Singleton
            val app = context.applicationContext as CfaitApplication
            val api = app.api

            AlarmScheduler.scheduleNextAlarm(context, api)
            Result.success()
        } catch (e: Exception) {
            Log.e("CfaitBootWorker", "Error", e)
            Result.retry()
        }
    }
}
