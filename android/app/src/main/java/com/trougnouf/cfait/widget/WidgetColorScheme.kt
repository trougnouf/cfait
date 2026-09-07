// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import androidx.compose.material3.darkColorScheme
import androidx.glance.color.ColorProviders
import androidx.glance.material3.ColorProviders

/**
 * A minimal dark color scheme for the widget that works on most home screens.
 * Glance does not support dynamic color from the system for widgets, so we
 * use a static dark scheme that adapts well to typical wallpapers.
 */
val WidgetColorScheme: ColorProviders = ColorProviders(
    darkColorScheme(
        primary = androidx.compose.ui.graphics.Color(0xFFB388FF),
        onPrimary = androidx.compose.ui.graphics.Color.Black,
        primaryContainer = androidx.compose.ui.graphics.Color(0xFF4E3390),
        onPrimaryContainer = androidx.compose.ui.graphics.Color(0xFFEADDFF),
        secondary = androidx.compose.ui.graphics.Color(0xFFCCC2DC),
        onSecondary = androidx.compose.ui.graphics.Color(0xFF332D41),
        secondaryContainer = androidx.compose.ui.graphics.Color(0xFF4A4458),
        onSecondaryContainer = androidx.compose.ui.graphics.Color(0xFFE8DEF8),
        tertiary = androidx.compose.ui.graphics.Color(0xFFEFB8C8),
        onTertiary = androidx.compose.ui.graphics.Color(0xFF492532),
        tertiaryContainer = androidx.compose.ui.graphics.Color(0xFF633B48),
        onTertiaryContainer = androidx.compose.ui.graphics.Color(0xFFFFD8E4),
        error = androidx.compose.ui.graphics.Color(0xFFF2B8B5),
        onError = androidx.compose.ui.graphics.Color(0xFF601410),
        errorContainer = androidx.compose.ui.graphics.Color(0xFF8C1D18),
        onErrorContainer = androidx.compose.ui.graphics.Color(0xFFF9DEDC),
        background = androidx.compose.ui.graphics.Color(0xFF1C1B1F),
        onBackground = androidx.compose.ui.graphics.Color(0xFFE6E1E5),
        surface = androidx.compose.ui.graphics.Color(0xFF1C1B1F),
        onSurface = androidx.compose.ui.graphics.Color(0xFFE6E1E5),
        surfaceVariant = androidx.compose.ui.graphics.Color(0xFF49454F),
        onSurfaceVariant = androidx.compose.ui.graphics.Color(0xFFCAC4D0),
        outline = androidx.compose.ui.graphics.Color(0xFF938F99),
        outlineVariant = androidx.compose.ui.graphics.Color(0xFF49454F),
        scrim = androidx.compose.ui.graphics.Color.Black,
    )
)
