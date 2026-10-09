// SPDX-License-Identifier: GPL-3.0-or-later
// Show toasts safely from any thread; a Looper is required to create a Toast,
// so calls arriving on a background thread are posted to the main thread.
package com.trougnouf.cfait.util

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.widget.Toast

private val mainHandler = Handler(Looper.getMainLooper())

fun showToast(context: Context?, message: String, duration: Int = Toast.LENGTH_SHORT) {
    val ctx = context ?: return
    if (Looper.myLooper() == Looper.getMainLooper()) {
        Toast.makeText(ctx, message, duration).show()
    } else {
        mainHandler.post { Toast.makeText(ctx, message, duration).show() }
    }
}
