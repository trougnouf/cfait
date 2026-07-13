// SPDX-License-Identifier: GPL-3.0-or-later
// Shared Compose UI components and syntax highlighting logic.
// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/Shared.kt
package com.trougnouf.cfait.ui

import android.content.Context
import android.app.Activity
import android.content.ContextWrapper
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Bundle
import android.os.Looper
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.style.TextOverflow
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileSyntaxType
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import kotlin.coroutines.resume

val NerdFont = FontFamily(Font(R.font.symbols_nerd_font))

fun parseInlineMarkdown(textStr: String, baseColor: androidx.compose.ui.graphics.Color, isStrikethrough: Boolean): androidx.compose.ui.text.AnnotatedString {
    val builder = androidx.compose.ui.text.AnnotatedString.Builder()
    val baseDecoration = if (isStrikethrough) androidx.compose.ui.text.style.TextDecoration.LineThrough else null

    var currentIdx = 0
    while (currentIdx < textStr.length) {
        val remaining = textStr.substring(currentIdx)

        val markers = listOf(
            Triple("<!-- uid:", "-->", 9),
            Triple("**", "**", 2),
            Triple("__", "__", 2),
            Triple("~~", "~~", 2),
            Triple("*", "*", 1),
            Triple("_", "_", 1),
            Triple("`", "`", 1)
        )

        var bestMatch: Triple<Int, Int, String>? = null
        var matchLen = 0
        var endLen = 0

        for ((startMarker, endMarker, sLen) in markers) {
            val startPos = remaining.indexOf(startMarker)
            if (startPos != -1) {
                val endPos = remaining.indexOf(endMarker, startPos + sLen)
                if (endPos != -1) {
                    val absStart = currentIdx + startPos
                    val eLen = endMarker.length
                    val absEnd = currentIdx + endPos + eLen
                    if (bestMatch == null || absStart < bestMatch.first) {
                        bestMatch = Triple(absStart, absEnd, startMarker)
                        matchLen = sLen
                        endLen = eLen
                    }
                }
            }
        }

        var searchIdx = 0
        while (true) {
            val startPos = remaining.indexOf('[', searchIdx)
            if (startPos == -1) break
            if (remaining.startsWith("[[", startPos)) {
                searchIdx = startPos + 2
                continue
            }
            val midPos = remaining.indexOf("](", startPos)
            if (midPos != -1) {
                val endPos = remaining.indexOf(')', midPos)
                if (endPos != -1) {
                    val absStart = currentIdx + startPos
                    val absEnd = currentIdx + endPos + 1
                    if (bestMatch == null || absStart < bestMatch.first) {
                        bestMatch = Triple(absStart, absEnd, "[]()")
                        matchLen = 0
                        endLen = 0
                    }
                    break
                }
            }
            searchIdx = startPos + 1
        }

        searchIdx = 0
        while (true) {
            val startPos = remaining.indexOf("[[", searchIdx)
            if (startPos == -1) break
            val endPos = remaining.indexOf("]]", startPos + 2)
            if (endPos != -1) {
                val absStart = currentIdx + startPos
                val absEnd = currentIdx + endPos + 2
                if (bestMatch == null || absStart < bestMatch.first) {
                    bestMatch = Triple(absStart, absEnd, "[[")
                    matchLen = 2
                    endLen = 2
                }
                break
            }
            searchIdx = startPos + 2
        }

        for (scheme in listOf("https://", "http://")) {
            val startPos = remaining.indexOf(scheme)
            if (startPos != -1) {
                val absStart = currentIdx + startPos
                var endOffset = 0
                for (c in textStr.substring(absStart)) {
                    if (c.isWhitespace() || c == ')' || c == ']') break
                    endOffset += 1
                }
                val absEnd = absStart + endOffset
                if (bestMatch == null || absStart < bestMatch.first) {
                    bestMatch = Triple(absStart, absEnd, "http")
                    matchLen = 0
                    endLen = 0
                }
            }
        }

        if (bestMatch != null) {
            val (absStart, absEnd, marker) = bestMatch

            if (absStart > currentIdx) {
                val chunk = textStr.substring(currentIdx, absStart)
                builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = baseColor, textDecoration = baseDecoration))
                builder.append(chunk)
                builder.pop()
            }

            val chunk = textStr.substring(absStart, absEnd)
            val innerChunk = if (matchLen > 0) textStr.substring(absStart + matchLen, absEnd - endLen) else chunk

            when (marker) {
                "<!-- uid:" -> {}
                "**", "__" -> {
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = baseColor, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold, textDecoration = baseDecoration))
                    builder.append(innerChunk)
                    builder.pop()
                }
                "*", "_" -> {
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = baseColor, fontStyle = androidx.compose.ui.text.font.FontStyle.Italic, textDecoration = baseDecoration))
                    builder.append(innerChunk)
                    builder.pop()
                }
                "~~" -> {
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = baseColor, textDecoration = androidx.compose.ui.text.style.TextDecoration.LineThrough))
                    builder.append(innerChunk)
                    builder.pop()
                }
                "`" -> {
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = androidx.compose.ui.graphics.Color(0xFFCC9966), fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace, textDecoration = baseDecoration))
                    builder.append(innerChunk)
                    builder.pop()
                }
                "[]()" -> {
                    val mid = chunk.indexOf("](")
                    val display = chunk.substring(1, mid)
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = androidx.compose.ui.graphics.Color(0xFF33B5E5), textDecoration = baseDecoration))
                    builder.append(display)
                    builder.pop()
                }
                "[[" -> {
                    val split = innerChunk.indexOf('|')
                    val display = if (split != -1) innerChunk.substring(split + 1) else innerChunk
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = androidx.compose.ui.graphics.Color(0xFF33B5E5), textDecoration = baseDecoration))
                    builder.append(display)
                    builder.pop()
                }
                "http" -> {
                    builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = androidx.compose.ui.graphics.Color(0xFF33B5E5), textDecoration = baseDecoration))
                    builder.append(chunk)
                    builder.pop()
                }
            }
            currentIdx = absEnd
        } else {
            break
        }
    }

    if (currentIdx < textStr.length) {
        builder.pushStyle(androidx.compose.ui.text.SpanStyle(color = baseColor, textDecoration = baseDecoration))
        builder.append(textStr.substring(currentIdx))
        builder.pop()
    }

    return builder.toAnnotatedString()
}

/**
 * Parses raw stringified Maps generated by the build script (e.g., "{one=1 item, other=X items}")
 * and extracts the correct pluralization string.
 */
fun resolvePluralMap(rawString: String, count: Int): String {
    if (rawString.trim().startsWith("{") && rawString.trim().endsWith("}")) {
        val oneMatch = Regex("""one=(.*?)(?:, \w+=|\})""").find(rawString)
        val otherMatch = Regex("""other=(.*?)(?:, \w+=|\})""").find(rawString)
        val zeroMatch = Regex("""zero=(.*?)(?:, \w+=|\})""").find(rawString)

        return when (count) {
            0 -> zeroMatch?.groupValues?.get(1) ?: otherMatch?.groupValues?.get(1) ?: rawString
            1 -> oneMatch?.groupValues?.get(1) ?: otherMatch?.groupValues?.get(1) ?: rawString
            else -> otherMatch?.groupValues?.get(1) ?: rawString
        }
    }
    return rawString
}

tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}

suspend fun fetchCurrentLocation(context: Context): Location? {
    val locationManager = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager
    var bestLocation: Location? = null
    try {
        for (provider in locationManager.getProviders(true)) {
            val l = locationManager.getLastKnownLocation(provider) ?: continue
            if (bestLocation == null || l.accuracy < bestLocation.accuracy) {
                bestLocation = l
            }
        }
    } catch (e: SecurityException) {
        // Ignore permission exceptions from disabled providers
    }

    // Return the cached location if it is less than 10 minutes old
    if (bestLocation != null && (System.currentTimeMillis() - bestLocation.time) < 10 * 60 * 1000) {
        return bestLocation
    }

    // Otherwise, attempt to fetch a fresh coordinate with a timeout
    return withTimeoutOrNull(5000) {
        suspendCancellableCoroutine<Location?> { cont ->
            try {
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    locationManager.getCurrentLocation(
                        LocationManager.NETWORK_PROVIDER,
                        null,
                        context.mainExecutor
                    ) { loc ->
                        if (cont.isActive) cont.resume(loc)
                    }
                } else {
                    val listener = object : LocationListener {
                        override fun onLocationChanged(loc: Location) {
                            if (cont.isActive) {
                                locationManager.removeUpdates(this)
                                cont.resume(loc)
                            }
                        }

                        override fun onStatusChanged(provider: String?, status: Int, extras: Bundle?) {}
                        override fun onProviderEnabled(provider: String) {}
                        override fun onProviderDisabled(provider: String) {}
                    }
                    locationManager.requestSingleUpdate(
                        LocationManager.NETWORK_PROVIDER,
                        listener,
                        Looper.getMainLooper()
                    )
                    cont.invokeOnCancellation { locationManager.removeUpdates(listener) }
                }
            } catch (e: Exception) {
                if (cont.isActive) cont.resume(bestLocation)
            }
        }
    } ?: bestLocation
}

object NfIcons {
    fun get(code: Int): String = String(Character.toChars(code))

    val CALENDARS_VIEW = get(0xf00f2)
    val TAGS_VIEW = get(0xf04fb)
    val LOCATION = get(0xef4b)
    val MAP_PIN = get(0xf276)
    val MAP = get(0xf279) // nf-fa-map (Filled)
    val MAP_MARKER_MULTIPLE = get(0xf1277) // nf-md-map_marker_multiple_outline
    val URL = get(0xf0c1)
    val GEO = get(0xf041)
    val CALENDAR = get(0xf073)
    val CALENDAR_CHECK = get(0xf274)
    val CALENDAR_XMARK = get(0xf273)
    val TAG = get(0xf02b)
    val TAG_OUTLINE = get(0xf04fc)
    val TAG_CHECK = get(0xf1a7a)
    val CHECK_CIRCLE = get(0xf058)
    val SETTINGS = get(0xe690)
    val REFRESH = get(0xf0450)
    val SYNC_ALERT = get(0xf04e7)
    val SYNC_OFF = get(0xf04e8)
    val DELETE = get(0xf1f8)
    val CHECK = get(0xf00c)
    val CHECK_SQUARE = get(0xf14a)
    val CROSS = get(0xf00d)
    val CHILD_ARROW = get(0xf149)
    val DETAILED_TRIANGLE = get(0xf01c6)

    val FAMILY_TREE = get(0xf160e)
    val EDIT_TREE = get(0xf1144)
    val TREE_FA = get(0xf1bb)
    val TREE_FAE = get(0xe21c)
    val TREE_MD = get(0xf0531)
    val PALM_TREE = get(0xf1055)
    val PINE_TREE = get(0xf0405)
    val PLUS_LOCK = get(0xf1a5d)

    val GOAL_ICONS = listOf(
        get(0xebf8), get(0xf04fe), get(0xf0a77), get(0xf4de),
        get(0xf11e), get(0xf023c), get(0xf140), get(0xf05dd),
        get(0xf08c9), get(0xf295), get(0xf1a04), get(0xf029a),
        get(0xf0873), get(0xf0874), get(0xf0875), get(0xf0995)
    )
    val LINK_LOCK = get(0xf10ba)
    val PLAY = get(0xeb2c)
    val PAUSE = get(0xf04c)
    val REPEAT = get(0xf0b6) // nf-fa-repeat_alt
    val REPEAT_VARIANT = get(0xf0547) // nf-md-repeat_variant
    val VISIBLE = get(0xea70)
    val HIDDEN = get(0xeae7)
    val WRITE_TARGET = get(0xf0cfb)
    val ADD = get(0xf067)
    val BACK = get(0xf060)
    val PRIORITY_UP = get(0xf0603)
    val PRIORITY_DOWN = get(0xf0604)
    val EDIT = get(0xf040)
    val ARROW_UP = get(0xf062)
    val ARROW_DOWN = get(0xf063)
    val HAND_STOP = get(0xf256)
    val FOCUS_FIELD = get(0xf0f4f)
    val THUMB_TACK = get(0xf08d)
    val ARROW_LEFT = get(0xf060)
    val ARROW_RIGHT = get(0xf061)
    val GONDOLA = get(0xf0686)
    val ROCKET_OUTLINE = get(0xf13af)
    val ELEVATOR_UP = get(0xf12c1)
    val ESCALATOR_UP = get(0xf12bf)
    val SAVE_AS = get(0xeb4a)
    // Extract Subtasks Icons (random variation)
    val SHOVEL = get(0xf0710) // nf-md-shovel
    val BULLDOZER = get(0xf0b22) // nf-md-bulldozer
    val PICKAXE = get(0xf08b7) // nf-md-pickaxe
    val LANGUAGE_MARKDOWN_OUTLINE = get(0xf0f5b) // nf-md-language_markdown_outline

    val ARROW_CIRCLE_UP = get(0xf0aa)
    val TRANSFER_UP = get(0xf0da3)
    val FLY = get(0xed43)
    val BALLOON = get(0xf0a26)
    val LINK = get(0xf0c1)
    val UNLINK = get(0xf127)
    val INFO = get(0xf129)
    val UNSYNCED = get(0xf0c2)
    val EXPORT = get(0xf093)
    val IMPORT = get(0xf019)
    val HELP = get(0xf0625)
    val BLOCKED = get(0xf479)
    val CHILD = get(0xf0a89)
    val CLONE = get(0xf24d)
    val HEART_HAND = get(0xed9b)
    val CREDIT_CARD = get(0xf09d)
    val BANK = get(0xf0a27)
    val BITCOIN = get(0xf10f)
    val LITECOIN = get(0xf0a61)
    val ETHEREUM = get(0xed58)
    val SEARCH = get(0xf002)
    val SEARCH_STOP = get(0xeb4e)
    val MENU = get(0xf0c9)
    val DOTS_CIRCLE = get(0xf1978)
    val COPY = get(0xf0c5)
    val EXTERNAL_LINK = get(0xf08e)
    val DEBUG_STOP = get(0xead7)
    val MOVE = get(0xef0c)
    val MAP_LOCATION_DOT = get(0xee69)
    val WEB_CHECK = get(0xf0789)
    val EARTH_ASIA = get(0xee47)
    val EARTH_AMERICAS = get(0xee46)
    val EARTH_AFRICA = get(0xee45)
    val EARTH_GENERIC = get(0xf01e7)
    val PLANET = get(0xe22e)
    val GALAXY = get(0xe243)
    val ISLAND = get(0xf104f)
    val COMPASS = get(0xebd5)
    val MOUNTAINS = get(0xe2a6)
    val GLOBE = get(0xf0ac)
    val GLOBEMODEL = get(0xf08e9)
    val BELL = get(0xf0f3)
    val HOURGLASS_START = get(0xf251)
    val HOURGLASS_END = get(0xf253)
    val TIMER_PLUS = get(0xf1ae3)

    // RANDOM_ICONS
    val DICE_D20 = get(0xeef5)
    val DICE_D20_DUP = get(0xeef5) // duplicate was present in Rust list
    val DICE_D6 = get(0xeef6)
    val DICE_MULTIPLE = get(0xf1156)
    val AUTO_FIX = get(0xf0068)
    val CRYSTAL_BALL = get(0xf0b2f)
    val ATOM = get(0xe27f)
    val CAT = get(0xeeed)
    val CAT_MD = get(0xf011b)
    val UNICORN = get(0xf15c2)
    val UNICORN_VARIANT = get(0xf15c3)
    val RAINBOW = get(0xef26)
    val FRUIT_CHERRIES = get(0xf1042)
    val FRUIT_PINEAPPLE = get(0xf1046)
    val FRUIT_PEAR = get(0xf1a0e)
    val DOG = get(0xf0a43)
    val PHOENIX = get(0xe860)
    val LINUX = get(0xf17c)
    val TORTOISE = get(0xf0d3b)
    val FACE_SMILE_WINK = get(0xeda9)
    val ROBOT_LOVE_OUTLINE = get(0xf16a6)
    val BOW_ARROW = get(0xf1841)
    val BULLSEYE_ARROW = get(0xf08c9)
    val COINS = get(0xe26b)
    val COW = get(0xeef1)
    val DOLPHIN = get(0xf18b4)
    val KIWI_BIRD = get(0xedff)
    val DUCK = get(0xf01e5)
    val FAE_TREE = get(0xe21c)
    val FA_TREE = get(0xf1bb)
    val MD_TREE = get(0xf0531)
    val PLANT = get(0xe22f)
    val WIZARD_HAT = get(0xf1477)
    val STAR_SHOOTING_OUTLINE = get(0xf1742)
    val WEATHER_STARS = get(0xe370)
    val KOALA = get(0xf173f)
    val SPIDER_THREAD = get(0xf11eb)
    val SQUIRREL = get(0xf483)
    val MUSHROOM_OUTLINE = get(0xf07e0)
    val FLOWER = get(0xf024a)
    val BEE_FLOWER = get(0xf0fa2)
    val LINUX_FREEBSD = get(0xf30c)
    val ARCHIVE_ARROW_UP = get(0xf125b)
    val BUG = get(0xf188)
    val WEATHER_SUNNY = get(0xf0599)
    val FROG = get(0xedf8)
    val BINOCULARS = get(0xf00a5)
    val ORANGE = get(0xe2a7)
    val SNOWMAN = get(0xef6a)
    val GNU = get(0xe779)
    val RUST = get(0xe7a8)
    val R_BOX = get(0xf0c1e)
    val PEPPER_HOT = get(0xef8b)
    val SIGN_POST = get(0xf277)
    val DATABASE = get(0xf01bc)
    val DATABASE_EYE_OUTLINE = get(0xf1922)

    val RELATED_FEMALE_FEMALE = get(0xf0a5a)
    val RELATED_MALE_MALE = get(0xf0a5e)
    val RELATED_MALE_FEMALE = get(0xf02e8)
}

fun getRandomExtractSubtasksIcon(): String {
    val icons = listOf(
        NfIcons.SHOVEL,
        NfIcons.BULLDOZER,
        NfIcons.PICKAXE,
        NfIcons.LANGUAGE_MARKDOWN_OUTLINE,
    )
    return icons.random()
}

fun getRandomRelatedIcon(
    uid1: String,
    uid2: String,
): String {
    val (first, second) =
        if (uid1 < uid2) {
            Pair(uid1, uid2)
        } else {
            Pair(uid2, uid1)
        }
    val hash = (first + second).fold(0) { acc, c -> (acc * 31 + c.code) and 0x7FFFFFFF }
    return when (hash % 3) {
        0 -> NfIcons.RELATED_FEMALE_FEMALE
        1 -> NfIcons.RELATED_MALE_MALE
        else -> NfIcons.RELATED_MALE_FEMALE
    }
}

fun getRandomScrollToTopIcon(): String {
    val icons =
        listOf(
            NfIcons.GONDOLA,
            NfIcons.ROCKET_OUTLINE,
            NfIcons.ELEVATOR_UP,
            NfIcons.ESCALATOR_UP,
            NfIcons.ARROW_CIRCLE_UP,
            NfIcons.TRANSFER_UP,
            NfIcons.FLY,
            NfIcons.BALLOON,
        )
    return icons.random()
}

@Composable
fun <T> DropdownPicker(
    label: String,
    selected: T,
    options: List<Pair<T, String>>,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier
) {
    val expanded = remember { mutableStateOf(false) }
    Box(modifier = modifier) {
        OutlinedCard(
            modifier = Modifier.fillMaxWidth().clickable { expanded.value = true }
        ) {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(options.find { it.first == selected }?.second ?: label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                NfIcon(NfIcons.ARROW_DOWN, 12.sp)
            }
        }
        DropdownMenu(
            expanded = expanded.value,
            onDismissRequest = { expanded.value = false }
        ) {
            options.forEach { (value, name) ->
                DropdownMenuItem(
                    text = { Text(name) },
                    onClick = { onSelect(value); expanded.value = false }
                )
            }
        }
    }
}

@Composable
fun NfIcon(
    text: String,
    size: TextUnit = 24.sp,
    color: Color = MaterialTheme.colorScheme.onSurface,
    lineHeight: TextUnit = TextUnit.Unspecified,
) {
    Text(text = text, fontFamily = NerdFont, fontSize = size, color = color, lineHeight = lineHeight)
}

fun parseHexColor(hex: String): Color =
    try {
        var clean = hex.removePrefix("#")
        if (clean.length > 6) {
            clean = clean.take(6)
        }
        val colorInt = android.graphics.Color.parseColor("#$clean")
        Color(colorInt)
    } catch (e: Exception) {
        Color.Gray
    }

@Composable
fun HeatmapRow(history: List<Float>) {
    Row(horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
        for (pct in history) {
            val color = when {
                pct >= 1.0f -> Color(0xFF4CAF50) // Green
                pct > 0f -> Color(0xFFFFB300) // Yellow
                else -> Color.DarkGray
            }
            Box(
                modifier = Modifier
                    .size(6.dp)
                    .background(color, RoundedCornerShape(1.dp))
            )
        }
    }
}

fun getTagColor(tag: String, isDark: Boolean): Color {
    val hash = tag.hashCode()
    val h = (kotlin.math.abs(hash) % 360).toFloat()

    // Dynamic Saturation and Value based on theme
    val s = if (isDark) 0.6f else 0.9f // Lower saturation in dark mode for better legibility
    val v = if (isDark) 0.9f else 0.4f // High brightness for dark mode, Low brightness for light mode

    return Color.hsv(h, s, v)
}


fun getTaskTextColor(
    prio: Int,
    isDone: Boolean,
    isDark: Boolean,
): Color {
    if (isDone) return Color.Gray
    return when (prio) {
        1 -> Color(0xFFFF4444)

        2 -> Color(0xFFFF6633)

        3 -> Color(0xFFFF8800)

        4 -> Color(0xFFFFBB33)

        5 -> Color(0xFFFFD700)

        6 -> Color(0xFFD9D98C)

        7 -> Color(0xFFB3BFC6)

        8 -> Color(0xFFA699CC)

        9 -> Color(0xFF998CA6)

        // Use Unspecified so Text() components use the active theme's onSurface color automatically
        else -> Color.Unspecified
    }
}

fun formatDuration(
    minMinutes: UInt,
    maxMinutes: UInt? = null,
    isEstimate: Boolean = false
): String {
    val min = minMinutes.toInt()

    fun fmt(m: Int): String =
        when {
            m >= 525600 -> "${m / 525600}y"
            m >= 43200 -> "${m / 43200}mo"
            m >= 10080 -> "${m / 10080}w"
            m >= 1440 -> "${m / 1440}d"
            m >= 60 -> "${m / 60}h"
            else -> "${m}m"
        }

    val minStr = fmt(min)

    // Only include the estimate prefix when appropriate
    val prefix = if (isEstimate) "~" else ""

    return if (maxMinutes != null && maxMinutes > minMinutes) {
        val maxStr = fmt(maxMinutes.toInt())
        "$prefix$minStr-$maxStr"
    } else {
        "$prefix$minStr"
    }
}

fun formatDurationHuman(mins: Long): String {
    if (mins == 0L) return "0m"
    val parts = mutableListOf<String>()
    var rem = mins
    val y = rem / 525600; if (y > 0) { parts.add("${y}y"); rem %= 525600 }
    val mo = rem / 43200; if (mo > 0) { parts.add("${mo}mo"); rem %= 43200 }
    val w = rem / 10080; if (w > 0) { parts.add("${w}w"); rem %= 10080 }
    val d = rem / 1440; if (d > 0) { parts.add("${d}d"); rem %= 1440 }
    val h = rem / 60; if (h > 0) { parts.add("${h}h"); rem %= 60 }
    if (rem > 0 || parts.isEmpty()) { parts.add("${rem}m") }
    return parts.joinToString(" ")
}

fun randomSessionExample(): String {
    val examples = listOf("30m", "1h", "2h", "6h", "14:00-15:30", "09:00-10:15")
    return examples.random()
}

fun formatPairedDuration(spentMins: Int, targetMins: Int): Pair<String, String> {
    val (cStr, tStr) = if (targetMins > 0 && targetMins % 1440 == 0) {
        val t = targetMins / 1440
        val c = spentMins.toFloat() / 1440f
        val formattedC = "%.1fd".format(java.util.Locale.US, c).replace(".0d", "d")
        Pair(formattedC, "${t}d")
    } else if (targetMins > 0 && targetMins % 60 == 0) {
        val t = targetMins / 60
        val c = spentMins.toFloat() / 60f
        val formattedC = "%.1fh".format(java.util.Locale.US, c).replace(".0h", "h")
        Pair(formattedC, "${t}h")
    } else {
        Pair("${spentMins}m", "${targetMins}m")
    }

    val cSuffix = cStr.filter { it.isLetter() }
    val tSuffix = tStr.filter { it.isLetter() }

    return if (cSuffix == tSuffix && cSuffix.isNotEmpty()) {
        val stripped = cStr.removeSuffix(cSuffix)
        Pair(stripped, tStr)
    } else {
        if (spentMins == 0) Pair("0", tStr) else Pair(cStr, tStr)
    }
}

private val timeFormatter = java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm")



fun formatIsoToLocal(isoString: String): String {
    return try {
        val instant = OffsetDateTime.parse(isoString).toInstant()
        val zone = ZoneId.systemDefault()
        instant.atZone(zone).format(timeFormatter)
    } catch (e: Exception) {
        // Fallback for malformed strings or unexpected formats
        isoString.take(16).replace("T", " ")
    }
}

class MarkdownTransformation(val isDark: Boolean) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val builder = AnnotatedString.Builder(raw)

        val headerColor = Color(0xFFFF9800) // Orange
        val linkColor = Color(0xFF33B5E5) // Cyan
        val dimColor = Color(0x80808080) // Gray
        val checkboxColor = Color(0xFF66BB6A) // Greenish
        val codeColor = Color(0xFFCC9966) // Brown/Orange

        var lineStart = 0
        val lines = raw.split('\n')
        for (line in lines) {
            val lineEnd = lineStart + line.length
            val trimmed = line.trimStart()

            if (trimmed.startsWith("#")) {
                builder.addStyle(SpanStyle(color = headerColor, fontWeight = FontWeight.Bold), lineStart, lineEnd)
            } else if (trimmed.startsWith("- [") || trimmed.startsWith("* [") || trimmed.startsWith("+ [") || Regex("^\\d+\\.\\s*\[").containsMatchIn(trimmed)) {
                val cbStart = line.indexOf('[')
                if (cbStart != -1 && cbStart + 2 < line.length && line[cbStart + 2] == ']') {
                    builder.addStyle(SpanStyle(color = checkboxColor), lineStart + cbStart, lineStart + cbStart + 3)
                }
            }

            lineStart = lineEnd + 1
        }

        try {
            val inlinePatterns = listOf(
                Pair(Regex("<!-- uid:.*?-->"), SpanStyle(color = dimColor, fontStyle = androidx.compose.ui.text.font.FontStyle.Italic)),
                Pair(Regex("\\[\\[.*?\\]\\]"), SpanStyle(color = linkColor, fontWeight = FontWeight.Bold)),
                Pair(Regex("\\[.*?\\]\(.*?\)"), SpanStyle(color = linkColor, fontWeight = FontWeight.Bold)),
                Pair(Regex("https?://[^\\s)\\]]+"), SpanStyle(color = linkColor, fontWeight = FontWeight.Bold)),
                Pair(Regex("\\*\\*.*?\\*\\*"), SpanStyle(fontWeight = FontWeight.Bold)),
                Pair(Regex("__.*?__"), SpanStyle(fontWeight = FontWeight.Bold)),
                Pair(Regex("~~.*?~~"), SpanStyle(textDecoration = androidx.compose.ui.text.style.TextDecoration.LineThrough)),
                Pair(Regex("(?<!\\*)\\*(?!\\*).*?(?<!\\*)\\*(?!\\*)"), SpanStyle(fontStyle = androidx.compose.ui.text.font.FontStyle.Italic)),
                Pair(Regex("(?<!_)_(?!_).*?(?<!_)_(?!_)"), SpanStyle(fontStyle = androidx.compose.ui.text.font.FontStyle.Italic)),
                Pair(Regex("`.*?`"), SpanStyle(color = codeColor, fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace))
            )

            for ((regex, style) in inlinePatterns) {
                regex.findAll(raw).forEach { match ->
                    builder.addStyle(style, match.range.first, match.range.last + 1)
                }
            }
        } catch (e: Exception) {
            // Ignore regex bounds errors to guarantee the text field never crashes
        }

        return TransformedText(builder.toAnnotatedString(), OffsetMapping.Identity)
    }
}

class SmartSyntaxTransformation(
    val api: CfaitMobile,
    val isDark: Boolean,
    val isSearch: Boolean = false
) : VisualTransformation {
    private val COLOR_DUE = Color(0xFF42A5F5)
    private val COLOR_START = Color(0xFF66BB6A)
    private val COLOR_RECUR = Color(0xFFAB47BC)
    private val COLOR_DURATION = Color(0xFF9E9E9E)
    private val COLOR_LOCATION = Color(0xFFFFB300)
    private val COLOR_URL = Color(0xFF4FC3F7)
    private val COLOR_META = Color(0xFF757575)
    private val COLOR_REMINDER = Color(0xFFFF7043)

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val builder = AnnotatedString.Builder(raw)

        try {
            val tokens = api.parseSmartString(raw, isSearch)

            for (token in tokens) {
                if (token.start >= raw.length || token.end > raw.length) continue

                val spanColor: Color? =
                    when (token.kind) {
                        MobileSyntaxType.PRIORITY -> {
                            val sub = raw.substring(token.start, token.end)
                            val p = sub.trimStart('!').toIntOrNull() ?: 0
                            getTaskTextColor(p, false, isDark)
                        }

                        MobileSyntaxType.DUE_DATE -> {
                            COLOR_DUE
                        }

                        MobileSyntaxType.START_DATE -> {
                            COLOR_START
                        }

                        MobileSyntaxType.RECURRENCE -> {
                            COLOR_RECUR
                        }

                        MobileSyntaxType.DURATION -> {
                            COLOR_DURATION
                        }

                        MobileSyntaxType.TAG -> {
                            val sub = raw.substring(token.start, token.end)
                            val tagName = sub.trimStart('#').replace("\"", "")
                            getTagColor(tagName, isDark)
                        }

                        MobileSyntaxType.LOCATION -> {
                            COLOR_LOCATION
                        }

                        MobileSyntaxType.URL -> {
                            COLOR_URL
                        }

                        MobileSyntaxType.GEO -> {
                            COLOR_META
                        }

                        MobileSyntaxType.DESCRIPTION -> {
                            COLOR_META
                        }

                        MobileSyntaxType.REMINDER -> {
                            COLOR_REMINDER
                        }

                        MobileSyntaxType.CALENDAR -> {
                            Color(0xFFE91E63) // Pink
                        }

                        MobileSyntaxType.FILTER -> {
                            Color(0xFF00BCD4)
                        }

                        MobileSyntaxType.OPERATOR -> {
                            Color(0xFFFF4081) // Pink/Magenta Accent
                        }

                        else -> {
                            null
                        }
                    }

                if (spanColor != null) {
                    val weight =
                        if (token.kind == MobileSyntaxType.PRIORITY
                            || token.kind == MobileSyntaxType.TAG
                            || token.kind == MobileSyntaxType.CALENDAR
                            || token.kind == MobileSyntaxType.OPERATOR
                        ) {
                            FontWeight.Bold
                        } else {
                            FontWeight.Normal
                        }

                    builder.addStyle(
                        SpanStyle(color = spanColor, fontWeight = weight),
                        token.start,
                        token.end,
                    )
                }
            }
        } catch (e: Exception) {
        }

        return TransformedText(builder.toAnnotatedString(), OffsetMapping.Identity)
    }
}

fun triggerBackgroundSync(context: Context, api: CfaitMobile) {
    CoroutineScope(Dispatchers.IO).launch {
        var errorMsg: String? = null
        try {
            api.syncJournal()
        } catch (e: Exception) {
            // Ignore network failures silently, the red sync icon will remain
            errorMsg = e.message
        } finally {
            val intent = android.content.Intent("com.trougnouf.cfait.REFRESH_UI")
            intent.putExtra("sync_error", errorMsg)
            intent.setPackage(context.packageName)
            context.sendBroadcast(intent)
        }
    }
}
