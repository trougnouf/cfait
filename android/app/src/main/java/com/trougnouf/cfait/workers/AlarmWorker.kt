// SPDX-License-Identifier: GPL-3.0-or-later
// Background worker for processing firing alarms.
package com.trougnouf.cfait.workers

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.trougnouf.cfait.CfaitApplication
import com.trougnouf.cfait.util.AlarmScheduler
import com.trougnouf.cfait.util.NotificationHelper
import com.trougnouf.cfait.widget.updateAllTaskListWidgets
import kotlinx.coroutines.CancellationException

class AlarmWorker(
    private val context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val UNIQUE_WORK_NAME = "cfait_alarm_processing"
    }

    override suspend fun doWork(): Result {
        return try {
            Log.d("CfaitAlarmWorker", "Starting alarm processing")

            val app = context.applicationContext as CfaitApplication
            // Wait for the background cache load before touching the store
            app.dataLoaded.await()
            val api = app.api

            // Try to sync first to check if alarms were dismissed elsewhere!
            try {
                api.sync()
            } catch (e: Exception) {
                Log.w("CfaitAlarmWorker", "Pre-alarm sync failed: ${e.message}")
                val intent = Intent(NotificationActionWorker.BROADCAST_REFRESH)
                intent.putExtra(NotificationActionWorker.KEY_SYNC_ERROR, e.message)
                intent.setPackage(context.packageName)
                context.sendBroadcast(intent)
            }

            val firing = api.getFiringAlarms()

            if (firing.isNotEmpty()) {
                Log.d("CfaitAlarmWorker", "Found ${firing.size} firing alarm(s)")

                val prefs = context.getSharedPreferences("cfait_fired_alarms", Context.MODE_PRIVATE)
                val recentlyFired = prefs.getStringSet("fired_uids", emptySet()) ?: emptySet()
                val newFired = mutableSetOf<String>()

                firing.forEach { alarm ->
                    newFired.add(alarm.alarmUid)
                    val isNew = !recentlyFired.contains(alarm.alarmUid)

                    NotificationHelper.showAlarmNotification(
                        context,
                        alarm.title,
                        alarm.body,
                        alarm.taskUid,
                        alarm.alarmUid,
                        isNew
                    )
                }

                prefs.edit().putStringSet("fired_uids", newFired).apply()
            } else {
                // Clean up prefs if no alarms are firing
                context.getSharedPreferences("cfait_fired_alarms", Context.MODE_PRIVATE)
                    .edit().remove("fired_uids").apply()
            }

            AlarmScheduler.scheduleNextAlarm(context, api)

            // The pre-alarm sync may have pulled in changes from other
            // clients; refresh the widgets even if the UI is not open.
            updateAllTaskListWidgets(context)

            Result.success()
        } catch (e: Exception) {
            if (e is CancellationException) throw e
            Log.e("CfaitAlarmWorker", "Error", e)
            Result.retry()
        }
    }
}
