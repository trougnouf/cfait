// SPDX-License-Identifier: GPL-3.0-or-later
// Evaluates location-based reminders (rem:@@loc) against the device position.
//
// Battery: this piggybacks on wake-ups that already happen (periodic sync,
// boot, app open). When no location reminders exist, hasLocationReminders()
// is false and we return before touching the location manager or permissions.
// We only read the last known location (a cached system value, no GPS fix).
package com.trougnouf.cfait.util

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationManager
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat
import com.trougnouf.cfait.BuildConfig
import com.trougnouf.cfait.core.CfaitMobile

object LocationReminderChecker {

    private const val TAG = "CfaitLocationReminder"

    /**
     * Evaluates all pending location reminders against the best last-known
     * device position and posts a notification for each one that fires
     * (enter/leave transition, at most once per 24h per reminder).
     */
    suspend fun check(context: Context, api: CfaitMobile) {
        try {
            // Cheap gate: no location reminders -> no work at all.
            if (!api.hasLocationReminders()) return
            val config = api.getConfig()
            if (!config.locationRemindersEnabled) return

            if (!hasLocationPermission(context)) {
                Log.d(TAG, "Location permission not granted; skipping location reminders")
                return
            }

            val loc = lastKnownLocation(context) ?: run {
                Log.d(TAG, "No last known location available")
                return
            }

            val fired = api.evaluateLocationReminders(loc.latitude, loc.longitude)
            if (fired.isNotEmpty()) {
                Log.i(TAG, "${fired.size} location reminder(s) fired")
                val prefs = context.getSharedPreferences("cfait_fired_alarms", Context.MODE_PRIVATE)
                val recentlyFired = prefs.getStringSet("fired_uids", emptySet()) ?: emptySet()
                val newFired = mutableSetOf<String>()
                for (alarm in fired) {
                    newFired.add(alarm.alarmUid)
                    NotificationHelper.showAlarmNotification(
                        context,
                        alarm.title,
                        alarm.body,
                        alarm.taskUid,
                        alarm.alarmUid,
                        !recentlyFired.contains(alarm.alarmUid)
                    )
                }
                prefs.edit().putStringSet("fired_uids", newFired).apply()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Location reminder check failed: ${e.message}")
        }
    }

    fun hasLocationPermission(context: Context): Boolean {
        val fine = ContextCompat.checkSelfPermission(
            context, Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
        val coarse = ContextCompat.checkSelfPermission(
            context, Manifest.permission.ACCESS_COARSE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
        return fine || coarse
    }

    fun hasBackgroundLocationPermission(context: Context): Boolean {
        // Play builds ship without ACCESS_BACKGROUND_LOCATION in the manifest,
        // so the check below would always fail; skip it explicitly.
        if (BuildConfig.PLAY_STORE_BUILD) return false
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return hasLocationPermission(context)
        return ContextCompat.checkSelfPermission(
            context, Manifest.permission.ACCESS_BACKGROUND_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Best last-known location across providers. A pure cache read: it does
     * not request a fresh fix, so it costs no battery.
     */
    private fun lastKnownLocation(context: Context): Location? {
        val locationManager = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager
        var best: Location? = null
        try {
            for (provider in locationManager.getProviders(true)) {
                val l = try {
                    locationManager.getLastKnownLocation(provider)
                } catch (e: SecurityException) {
                    null
                } ?: continue
                if (best == null || l.accuracy < best.accuracy) best = l
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to read last known location: ${e.message}")
        }
        return best
    }
}
