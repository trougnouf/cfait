// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./android/app/src/main/java/com/trougnouf/cfait/util/NotificationHelper.kt
package com.trougnouf.cfait.util

import android.Manifest
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.trougnouf.cfait.MainActivity
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileTask
import com.trougnouf.cfait.receivers.NotificationActionReceiver
import com.trougnouf.cfait.widget.EXTRA_FOCUS_TASK_UID
import com.trougnouf.cfait.workers.NotificationActionWorker

object NotificationHelper {
    fun updateOngoingNotifications(context: Context, api: CfaitMobile) {
        if (ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return
        }

        val config = api.getConfig()
        val ongoingTasks = api.getOngoingTasks()
        val prefs = context.getSharedPreferences("cfait_ongoing_notifs", Context.MODE_PRIVATE)

        // Cleanup dismissed flags for tasks no longer ongoing
        val ongoingUids = ongoingTasks.map { task -> task.uid }.toSet()
        val stale = prefs.all.keys.filter { !ongoingUids.contains(it) }
        if (stale.isNotEmpty()) {
            val editor = prefs.edit()
            stale.forEach { editor.remove(it) }
            editor.apply()
        }

        val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        if (!config.showOngoingNotifications) {
            return // Obsolete notifications are cleaned up by AlarmScheduler
        }

        val activeIds = notificationManager.activeNotifications.map { it.id }.toSet()
        ongoingTasks.forEach { task ->
            // Check if user explicitly swiped this notification away
            if (prefs.getBoolean(task.uid, false)) return@forEach
            // The chronometer keeps running on its own; re-posting the same
            // notification would only collapse an expanded one and spam the tray.
            if (activeIds.contains(task.uid.hashCode())) return@forEach
            showActiveTaskNotification(context, task)
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
            putExtra(NotificationActionWorker.EXTRA_TASK_UID, task.uid)
            putExtra(NotificationActionWorker.EXTRA_ALARM_UID, originalAlarmUid ?: "")
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
            putExtra(NotificationActionWorker.EXTRA_TASK_UID, task.uid)
            putExtra(NotificationActionWorker.EXTRA_ALARM_UID, originalAlarmUid ?: "")
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
            putExtra(NotificationActionWorker.EXTRA_TASK_UID, task.uid)
            putExtra(NotificationActionWorker.EXTRA_ALARM_UID, originalAlarmUid ?: "")
        }
        val dismissPending = PendingIntent.getBroadcast(
            context,
            (task.uid + "DISMISS_ONGOING").hashCode(),
            dismissIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Tap opens app
        val tapIntent = Intent(context, MainActivity::class.java).apply {
            putExtra(EXTRA_FOCUS_TASK_UID, task.uid)
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
            .addExtras(Bundle().apply {
                putString("cfait_task_uid", task.uid)
                putString("cfait_notif_type", "ongoing")
            })
            .build()

        val notificationManager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(notificationId, notification)
    }

    /**
     * Widgets cannot show toasts, so failed in-widget actions surface here as a
     * quiet, dismissible notification on the status channel.
     */
    fun showWidgetErrorNotification(context: Context) {
        try {
            val notificationId = "widget_error".hashCode()
            val tapIntent = Intent(context, MainActivity::class.java)
            val tapPending = PendingIntent.getActivity(
                context,
                notificationId,
                tapIntent,
                PendingIntent.FLAG_IMMUTABLE
            )
            val notification = NotificationCompat.Builder(context, NotificationActionWorker.CHANNEL_STATUS)
                .setSmallIcon(R.drawable.ic_launcher_foreground)
                .setContentTitle(context.getString(R.string.app_name))
                .setContentText(context.getString(R.string.widget_action_failed))
                .setOnlyAlertOnce(true)
                .setAutoCancel(true)
                .setContentIntent(tapPending)
                .build()
            NotificationManagerCompat.from(context).notify(notificationId, notification)
        } catch (e: SecurityException) {
            // Notification permission was revoked; nothing to do.
        }
    }
}
