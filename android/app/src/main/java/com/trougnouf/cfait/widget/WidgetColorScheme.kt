// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.widget

import androidx.compose.material3.darkColorScheme
import androidx.compose.ui.graphics.Color
import androidx.glance.color.ColorProviders
import androidx.glance.material3.ColorProviders

/**
 * A minimal dark color scheme for the widget that works on most home screens.
 * Glance does not support dynamic color from the system for widgets, so we
 * use a static dark scheme that adapts well to typical wallpapers.
 */
val WidgetColorScheme: ColorProviders = ColorProviders(
    darkColorScheme(
        primary = Color(0xFFB388FF),
        onPrimary = Color.Black,
        primaryContainer = Color(0xFF4E3390),
        onPrimaryContainer = Color(0xFFEADDFF),
        secondary = Color(0xFFCCC2DC),
        onSecondary = Color(0xFF332D41),
        secondaryContainer = Color(0xFF4A4458),
        onSecondaryContainer = Color(0xFFE8DEF8),
        tertiary = Color(0xFFEFB8C8),
        onTertiary = Color(0xFF492532),
        tertiaryContainer = Color(0xFF633B48),
        onTertiaryContainer = Color(0xFFFFD8E4),
        error = Color(0xFFF2B8B5),
        onError = Color(0xFF601410),
        errorContainer = Color(0xFF8C1D18),
        onErrorContainer = Color(0xFFF9DEDC),
        background = Color(0xFF1C1B1F),
        onBackground = Color(0xFFE6E1E5),
        surface = Color(0xFF1C1B1F),
        onSurface = Color(0xFFE6E1E5),
        surfaceVariant = Color(0xFF49454F),
        onSurfaceVariant = Color(0xFFCAC4D0),
        outline = Color(0xFF938F99),
        outlineVariant = Color(0xFF49454F),
        scrim = Color.Black,
    )
)
