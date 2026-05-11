// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./android/app/src/main/java/com/trougnouf/cfait/util/NotificationHelper.kt
package com.trougnouf.cfait.util

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileTask
import com.trougnouf.cfait.receivers.NotificationActionReceiver
import com.trougnouf.cfait.workers.NotificationActionWorker

object NotificationHelper {
    fun updateOngoingNotifications(context: Context, api: CfaitMobile) {
        if (androidx.core.content.ContextCompat.checkSelfPermission(
                context,
                android.Manifest.permission.POST_NOTIFICATIONS
            ) != android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            return
        }

        val config = api.getConfig()
        val ongoingTasks = api.getOngoingTasks()
        val prefs = context.getSharedPreferences("cfait_ongoing_notifs", Context.MODE_PRIVATE)

        // Cleanup dismissed flags for tasks no longer ongoing
        val ongoingUids = ongoingTasks.map { task -> task.uid }.toSet()
        for (key in prefs.all.keys) {
            if (!ongoingUids.contains(key)) {
                prefs.edit().remove(key).apply()
            }
        }

        val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        if (!config.showOngoingNotifications) {
            return // Obsolete notifications are cleaned up by AlarmScheduler
        }

        ongoingTasks.forEach { task ->
            // Check if user explicitly swiped this notification away
            if (!prefs.getBoolean(task.uid, false)) {
                showActiveTaskNotification(context, task)
            }
        }
    }

    fun showActiveTaskNotification(
        context: Context,
        task: MobileTask,
        originalAlarmUid: String? = null
    ) {
        val notificationId = task.uid.hashCode()

        // Calculate base time for Chronometer
        val now = System.currentTimeMillis()
        val startTs = task.lastStartedAt ?: (now / 1000)
        val currentSessionMs = (now - (startTs * 1000))
        val totalSpentMs = (task.timeSpentSeconds.toLong() * 1000) + currentSessionMs

        // Pause Action
        val pauseIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            this.action = NotificationActionWorker.ACTION_PAUSE
            putExtra("T_UID", task.uid)
            putExtra("A_UID", originalAlarmUid ?: "")
        }
        val pausePending = PendingIntent.getBroadcast(
            context,
            (task.uid + "PAUSE").hashCode(),
            pauseIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Done Action
        val doneIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            this.action = NotificationActionWorker.ACTION_DONE
            putExtra("T_UID", task.uid)
            putExtra("A_UID", originalAlarmUid ?: "")
        }
        val donePending = PendingIntent.getBroadcast(
            context,
            (task.uid + "DONE_ACT").hashCode(),
            doneIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Dismiss Action (swiping away)
        val dismissIntent = Intent(context, NotificationActionReceiver::class.java).apply {
            this.action = NotificationActionWorker.ACTION_DISMISS_ONGOING
            putExtra("T_UID", task.uid)
            putExtra("A_UID", originalAlarmUid ?: "")
        }
        val dismissPending = PendingIntent.getBroadcast(
            context,
            (task.uid + "DISMISS_ONGOING").hashCode(),
            dismissIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Tap opens app
        val tapIntent = Intent(context, MainActivity::class.java).apply {
            putExtra("focus_task_uid", task.uid)
        }
        val tapPending =
            PendingIntent.getActivity(context, task.uid.hashCode(), tapIntent, PendingIntent.FLAG_IMMUTABLE)

        val notification = NotificationCompat.Builder(context, NotificationActionWorker.CHANNEL_ALARMS)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(context.getString(R.string.notification_in_progress, task.summary))
            .setUsesChronometer(true)
            .setWhen(System.currentTimeMillis() - totalSpentMs)
            .setShowWhen(true)
            .setOnlyAlertOnce(true)
            .setOngoing(false) // Allow swipe
            .setDeleteIntent(dismissPending)
            .setContentIntent(tapPending)
            .addAction(R.drawable.ic_launcher_foreground, context.getString(R.string.pause), pausePending)
            .addAction(R.drawable.ic_launcher_foreground, context.getString(R.string.done), donePending)
            .addExtras(android.os.Bundle().apply {
                putString("cfait_task_uid", task.uid)
                putString("cfait_notif_type", "ongoing")
            })
            .build()

        val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(notificationId, notification)
    }
}
