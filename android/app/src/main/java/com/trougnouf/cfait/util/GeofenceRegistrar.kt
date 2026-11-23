// SPDX-License-Identifier: GPL-3.0-or-later
// Registers system-level geofences (LocationManager.addProximityAlert) for
// every pending location reminder that has coordinates configured.
//
// Proximity alerts are serviced by the Android system location stack (geofence
// hardware where available): they fire near-instantly and cost ~no battery,
// unlike polling. They need ACCESS_BACKGROUND_LOCATION to fire while the app
// is closed, and they do not survive a reboot, so re-register on boot, after
// every sync, on app foreground, and whenever reminders/coords change.
//
// Google Play builds ship without ACCESS_BACKGROUND_LOCATION (policy), so
// registration is skipped there; reminders are evaluated on app open only.
package com.trougnouf.cfait.util

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.location.LocationManager
import android.util.Log
import com.trougnouf.cfait.BuildConfig
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.receivers.ProximityAlertReceiver

object GeofenceRegistrar {

    private const val TAG = "CfaitGeofence"
    private const val PREFS = "cfait_geofences"
    const val EXTRA_GEOFENCE_KEY = "cfait_geofence_key"

    /** key format: "name|lat,lon|radiusM" */
    fun keyFor(name: String, lat: Double, lon: Double, radiusM: UInt): String =
        "$name|$lat,$lon|$radiusM"

    private fun pendingIntentFor(context: Context, key: String): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            key.hashCode(),
            Intent(context, ProximityAlertReceiver::class.java).putExtra(EXTRA_GEOFENCE_KEY, key),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

    fun register(context: Context, api: CfaitMobile) {
        if (BuildConfig.PLAY_STORE_BUILD || !LocationReminderChecker.hasBackgroundLocationPermission(context)) {
            removeAll(context)
            return
        }
        val config = api.getConfig()
        if (!config.locationRemindersEnabled) {
            removeAll(context)
            return
        }
        val coords = config.locationCoords
        // One alert per (location, resolved radius) pair so per-reminder radii stay exact.
        val wanted = mutableMapOf<String, Triple<Double, Double, Float>>()
        for (rem in api.getLocationReminders()) {
            for (name in rem.locations) {
                val c = coords[name] ?: continue
                val radius = rem.radiusM ?: c.radiusM ?: config.defaultLocationRadiusM
                val key = keyFor(name, c.lat, c.lon, radius)
                wanted[key] = Triple(c.lat, c.lon, radius.toFloat())
            }
        }

        val lm = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val registered = prefs.getStringSet("keys", emptySet()) ?: emptySet()

        for (stale in registered - wanted.keys) {
            try {
                lm.removeProximityAlert(pendingIntentFor(context, stale))
            } catch (e: Exception) {
                Log.w(TAG, "Failed to remove stale geofence $stale: ${e.message}")
            }
        }
        for ((key, t) in wanted) {
            if (key in registered) continue
            try {
                // expiration = -1 (never); alerts are re-registered on boot anyway.
                lm.addProximityAlert(t.first, t.second, t.third, -1L, pendingIntentFor(context, key))
                Log.i(TAG, "Registered geofence $key")
            } catch (e: SecurityException) {
                Log.w(TAG, "No permission to register geofence $key")
            } catch (e: Exception) {
                Log.w(TAG, "Failed to register geofence $key: ${e.message}")
            }
        }
        prefs.edit().putStringSet("keys", wanted.keys).apply()
    }

    fun removeAll(context: Context) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val registered = prefs.getStringSet("keys", emptySet()) ?: emptySet()
        if (registered.isEmpty()) return
        val lm = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager
        for (key in registered) {
            try {
                lm.removeProximityAlert(pendingIntentFor(context, key))
            } catch (_: Exception) {
            }
        }
        prefs.edit().remove("keys").apply()
    }
}
