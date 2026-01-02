// File: ./android/app/src/main/java/com/cfait/workers/CalendarMigrationWorker.kt
package com.cfait.workers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.WorkerParameters
import com.cfait.CfaitApplication

class CalendarMigrationWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val KEY_SOURCE_HREF = "source_href"
        const val KEY_TARGET_HREF = "target_href"
        const val OUTPUT_MESSAGE = "message"
        const val UNIQUE_WORK_NAME = "cfait_migration"
    }

    override suspend fun doWork(): Result {
        val sourceHref = inputData.getString(KEY_SOURCE_HREF) ?: return Result.failure()
        val targetHref = inputData.getString(KEY_TARGET_HREF) ?: return Result.failure()

        Log.d("CfaitMigrate", "Starting migration from $sourceHref to $targetHref")

        val app = applicationContext as CfaitApplication
        val api = app.api

        return try {
            // This calls the Rust function which already handles concurrency
            val resultMessage = api.migrateLocalTo(sourceHref, targetHref)

            Log.d("CfaitMigrate", "Migration success: $resultMessage")

            val output = Data.Builder()
                .putString(OUTPUT_MESSAGE, resultMessage)
                .build()

            Result.success(output)
        } catch (e: Exception) {
            Log.e("CfaitMigrate", "Migration failed", e)
            val output = Data.Builder()
                .putString(OUTPUT_MESSAGE, "Migration error: ${e.message}")
                .build()
            Result.failure(output)
        }
    }
}
