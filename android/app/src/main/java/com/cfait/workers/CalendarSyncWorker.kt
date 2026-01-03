// Worker for bulk calendar event operations.
package com.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.WorkerParameters
import com.cfait.CfaitApplication

class CalendarSyncWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val KEY_MODE = "mode"
        const val MODE_CREATE = "create" // Backfill
        const val MODE_DELETE = "delete" // Delete All
        const val OUTPUT_COUNT = "count"
        const val OUTPUT_MESSAGE = "message"

        const val UNIQUE_WORK_NAME = "cfait_calendar_bulk_sync"
    }

    override suspend fun doWork(): Result {
        val mode = inputData.getString(KEY_MODE) ?: return Result.failure()

        Log.d("CfaitCalSync", "Starting bulk calendar operation: $mode")

        val app = applicationContext as CfaitApplication
        val api = app.api

        return try {
            val count = when (mode) {
                MODE_CREATE -> api.createMissingCalendarEvents()
                MODE_DELETE -> api.deleteAllCalendarEvents()
                else -> 0u
            }

            val actionName = if (mode == MODE_CREATE) "Created" else "Deleted"
            val message = "$actionName $count calendar event${if (count == 1u) "" else "s"}"

            Log.d("CfaitCalSync", "Success: $message")

            val output = Data.Builder()
                .putInt(OUTPUT_COUNT, count.toInt())
                .putString(OUTPUT_MESSAGE, message)
                .build()

            Result.success(output)
        } catch (e: Exception) {
            Log.e("CfaitCalSync", "Failed to sync calendar events", e)
            val output = Data.Builder()
                .putString(OUTPUT_MESSAGE, "Error: ${e.message}")
                .build()
            Result.failure(output)
        }
    }
}
