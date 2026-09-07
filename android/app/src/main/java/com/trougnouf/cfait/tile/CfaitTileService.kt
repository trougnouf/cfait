// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.tile

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import com.trougnouf.cfait.MainActivity

/**
 * Quick Settings tile that launches Cfait with a single tap.
 *
 * Accessible from the lock screen by swiping down the QS panel, saving the
 * unlock-to-app navigation step for quick task capture.
 */
class CfaitTileService : TileService() {

    override fun onStartListening() {
        super.onStartListening()
        // The tile is a pure launcher, so it stays "active" (colored) at all times.
        qsTile?.apply {
            state = Tile.STATE_ACTIVE
            updateTile()
        }
    }

    override fun onClick() {
        super.onClick()
        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val pi = PendingIntent.getActivity(
                this, 0, intent, PendingIntent.FLAG_IMMUTABLE
            )
            startActivityAndCollapse(pi)
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }
}
