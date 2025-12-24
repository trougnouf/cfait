// File: android/app/src/main/java/com/cfait/ui/Shared.kt
package com.cfait.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import com.cfait.R
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileSyntaxType

val NerdFont = FontFamily(Font(R.font.symbols_nerd_font))

object NfIcons {
    fun get(code: Int): String = String(Character.toChars(code))

    val CALENDARS_VIEW = get(0xf00f2)
    val TAGS_VIEW = get(0xf04fb)
    val LOCATION = get(0xef4b)
    val MAP_PIN = get(0xf276)
    val MAP = get(0xec05)
    val URL = get(0xf0c1)
    val GEO = get(0xf041)
    val CALENDAR = get(0xf073)
    val TAG = get(0xf02b)
    val SETTINGS = get(0xe690)
    val REFRESH = get(0xf0450)
    val SYNC_ALERT = get(0xf04e7)
    val SYNC_OFF = get(0xf04e8)
    val DELETE = get(0xf1f8)
    val CHECK = get(0xf00c)
    val CROSS = get(0xf00d)
    val PLAY = get(0xeb2c)
    val PAUSE = get(0xf04c)
    val REPEAT = get(0xf0b6)
    val VISIBLE = get(0xea70)
    val HIDDEN = get(0xeae7)
    val WRITE_TARGET = get(0xf0cfb)
    val ADD = get(0xf067)
    val BACK = get(0xf060)
    val PRIORITY_UP = get(0xf0603)
    val PRIORITY_DOWN = get(0xf0604)
    val EDIT = get(0xf040)
    val ARROW_RIGHT = get(0xf061)
    val LINK = get(0xf0c1)
    val UNLINK = get(0xf127)
    val INFO = get(0xf129)
    val UNSYNCED = get(0xf0c2)
    val EXPORT = get(0xeac3)
    val HELP = get(0xf0625)
    val BLOCKED = get(0xf479)
    val CHILD = get(0xf0a89)
    val HEART_HAND = get(0xed9b)
    val CREDIT_CARD = get(0xf09d)
    val BANK = get(0xf0a27)
    val BITCOIN = get(0xf10f)
    val LITECOIN = get(0xf0a61)
    val ETHEREUM = get(0xed58)
    val SEARCH = get(0xf002)
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
}

@Composable
fun NfIcon(
    text: String,
    size: TextUnit = 24.sp,
    color: Color = MaterialTheme.colorScheme.onSurface,
) {
    Text(text = text, fontFamily = NerdFont, fontSize = size, color = color)
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

fun getTagColor(tag: String): Color {
    val hash = tag.hashCode()
    val h = (kotlin.math.abs(hash) % 360).toFloat()
    return Color.hsv(h, 0.6f, 0.5f)
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
        else -> if (isDark) Color.White else Color.Black
    }
}

fun formatDuration(minutes: UInt): String {
    val m = minutes.toInt()
    return when {
        m >= 525600 -> "~${m / 525600}y"
        m >= 43200 -> "~${m / 43200}mo"
        m >= 10080 -> "~${m / 10080}w"
        m >= 1440 -> "~${m / 1440}d"
        m >= 60 -> "~${m / 60}h"
        else -> "~${m}m"
    }
}

class SmartSyntaxTransformation(
    val api: CfaitMobile,
    val isDark: Boolean,
) : VisualTransformation {

    // Define colors once
    private val COLOR_DUE = Color(0xFF42A5F5)
    private val COLOR_START = Color(0xFF66BB6A)
    private val COLOR_RECUR = Color(0xFFAB47BC)
    private val COLOR_DURATION = Color(0xFF9E9E9E)
    private val COLOR_LOCATION = Color(0xFFFFB300)
    private val COLOR_URL = Color(0xFF4FC3F7)
    private val COLOR_META = Color(0xFF757575)

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val builder = AnnotatedString.Builder(raw)

        // Offload parsing to Rust Core
        // Note: VisualTransformation runs on UI thread.
        // Rust string parsing is extremely fast (μs range), so blocking here is acceptable.
        try {
            val tokens = api.parseSmartString(raw)

            for (token in tokens) {
                // Ensure indices are within bounds (Rust UTF-16 mapping logic handles emoji,
                // but safety check prevents crashes during race conditions or weird inputs)
                if (token.start >= raw.length || token.end > raw.length) continue

                val spanColor: Color? =
                    when (token.kind) {
                        MobileSyntaxType.PRIORITY -> {
                            // Extract number from text for priority color
                            val sub = raw.substring(token.start.toInt(), token.end.toInt())
                            val p = sub.trimStart('!').toIntOrNull() ?: 0
                            getTaskTextColor(p, false, isDark)
                        }

                        MobileSyntaxType.DUE_DATE -> COLOR_DUE
                        MobileSyntaxType.START_DATE -> COLOR_START
                        MobileSyntaxType.RECURRENCE -> COLOR_RECUR
                        MobileSyntaxType.DURATION -> COLOR_DURATION
                        MobileSyntaxType.TAG -> {
                            val sub = raw.substring(token.start.toInt(), token.end.toInt())
                            val tagName = sub.trimStart('#').replace("\"", "")
                            getTagColor(tagName)
                        }

                        MobileSyntaxType.LOCATION -> COLOR_LOCATION
                        MobileSyntaxType.URL -> COLOR_URL
                        MobileSyntaxType.GEO -> COLOR_META
                        MobileSyntaxType.DESCRIPTION -> COLOR_META
                        else -> null
                    }

                if (spanColor != null) {
                    val weight =
                        if (token.kind == MobileSyntaxType.PRIORITY || token.kind == MobileSyntaxType.TAG) {
                            FontWeight.Bold
                        } else {
                            FontWeight.Normal
                        }

                    builder.addStyle(
                        SpanStyle(color = spanColor, fontWeight = weight),
                        token.start.toInt(),
                        token.end.toInt(),
                    )
                }
            }
        } catch (e: Exception) {
            // Fallback to plain text if Rust bridge fails (rare)
        }

        return TransformedText(builder.toAnnotatedString(), OffsetMapping.Identity)
    }
}
