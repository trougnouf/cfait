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
import java.util.regex.Pattern

val NerdFont = FontFamily(Font(R.font.symbols_nerd_font))

object NfIcons {
    fun get(code: Int): String = String(Character.toChars(code))

    // --- UPDATED ICONS ---
    val CALENDARS_VIEW = get(0xf00f2) // nf-md-calendar_multiple_check
    val TAGS_VIEW = get(0xf04fb) // nf-md-tag_multiple
    val LOCATION = get(0xef4b) // nf-fa-earth_europe
    val MAP_PIN = get(0xf276) // nf-fa-map_pin
    val MAP = get(0xec05) // nf-cod-map
    val URL = get(0xf0c1)
    val GEO = get(0xf041)
    // --------------------

    val CALENDAR = get(0xf073) // 
    val TAG = get(0xf02b) // 
    val SETTINGS = get(0xe690) // nf-seti-settings
    val REFRESH = get(0xf0450) // nf-md-refresh
    val SYNC_ALERT = get(0xf04e7) // nf-md-sync_alert
    val SYNC_OFF = get(0xf04e8) // nf-md-sync_off
    val DELETE = get(0xf1f8) // 
    val CHECK = get(0xf00c) // 
    val CROSS = get(0xf00d) // 
    val PLAY = get(0xeb2c) // nf-cod-play
    val PAUSE = get(0xf04c) // 
    val REPEAT = get(0xf0b6) // 
    val VISIBLE = get(0xea70) // nf-cod-eye
    val HIDDEN = get(0xeae7) // nf-cod-eye_closed
    val WRITE_TARGET = get(0xf0cfb) // nf-md-content_save_edit
    val ADD = get(0xf067) // +
    val BACK = get(0xf060) // 
    val PRIORITY_UP = get(0xf0603) // nf-md-priority_high
    val PRIORITY_DOWN = get(0xf0604) // nf-md-priority_low
    val EDIT = get(0xf040) // 
    val ARROW_RIGHT = get(0xf061) // 
    val LINK = get(0xf0c1) // 
    val UNLINK = get(0xf127) // 
    val INFO = get(0xf129) // 
    val UNSYNCED = get(0xf0c2) // 
    val EXPORT = get(0xeac3) // 
    val HELP = get(0xf0625) // 󰘥

    val BLOCKED = get(0xf479) // nf-oct-blocked
    val CHILD = get(0xf0a89) // nf-md-account_child

    val HEART_HAND = get(0xed9b)
    val CREDIT_CARD = get(0xf09d)
    val BANK = get(0xf0a27)
    val BITCOIN = get(0xf10f)
    val LITECOIN = get(0xf0a61)
    val ETHEREUM = get(0xed58)

    val SEARCH = get(0xf002) // 
    val MENU = get(0xf0c9) // 
    val DOTS_CIRCLE = get(0xf1978)
    val COPY = get(0xf0c5)
    val EXTERNAL_LINK = get(0xf08e)
    val DEBUG_STOP = get(0xead7)
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
    val isDark: Boolean,
) : VisualTransformation {
    // ... [existing validator helper methods] ...
    private fun isValidDateUnit(s: String): Boolean {
        val lower = s.lowercase()
        return when (lower) {
            "week", "month", "year",
            "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
            -> true

            else -> false
        }
    }

    private fun isValidDurationUnit(s: String): Boolean {
        val lower = s.lowercase()
        return lower == "d" || lower == "day" || lower == "days" ||
            lower == "w" || lower == "week" || lower == "weeks" ||
            lower == "mo" || lower == "month" || lower == "months" ||
            lower == "y" || lower == "year" || lower == "years"
    }

    private fun isValidSmartDate(s: String): Boolean {
        val lower = s.lowercase()
        if (lower == "today" || lower == "tomorrow") return true
        if (lower.matches(Regex("\\d{4}-\\d{2}-\\d{2}"))) return true

        fun check(suffix: String) = lower.endsWith(suffix) && lower.removeSuffix(suffix).toLongOrNull() != null
        return check("d") || check("w") || check("y") || check("mo")
    }

    private fun isValidDuration(s: String): Boolean {
        val lower = s.lowercase()

        fun check(suffix: String) = lower.endsWith(suffix) && lower.removeSuffix(suffix).toLongOrNull() != null
        return check("min") || check("m") || check("h") || check("d") || check("w") || check("mo") || check("y")
    }

    private fun isValidRecurrence(s: String): Boolean {
        val lower = s.lowercase()
        return lower == "daily" || lower == "weekly" || lower == "monthly" || lower == "yearly"
    }

    private fun isValidFreqUnit(s: String): Boolean {
        val lower = s.lowercase()
        return lower.startsWith("day") || lower.startsWith("week") || lower.startsWith("month") || lower.startsWith("year")
    }

    private val COLOR_DUE = Color(0xFF42A5F5)
    private val COLOR_START = Color(0xFF66BB6A)
    private val COLOR_RECUR = Color(0xFFAB47BC)
    private val COLOR_DURATION = Color(0xFF9E9E9E)

    // --- NEW COLORS ---
    private val COLOR_LOCATION = Color(0xFFFFB300) // Amber
    private val COLOR_URL = Color(0xFF4FC3F7) // Light Blue
    private val COLOR_META = Color(0xFF757575) // Grey for Desc/Geo

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val builder = AnnotatedString.Builder(raw)

        val pattern = Pattern.compile("\\s+")
        val matcher = pattern.matcher(raw)

        val words = mutableListOf<Triple<Int, Int, String>>()
        var lastEnd = 0

        while (matcher.find()) {
            val start = matcher.start()
            if (start > lastEnd) {
                words.add(Triple(lastEnd, start, raw.substring(lastEnd, start)))
            }
            lastEnd = matcher.end()
        }
        if (lastEnd < raw.length) {
            words.add(Triple(lastEnd, raw.length, raw.substring(lastEnd)))
        }

        var i = 0
        while (i < words.size) {
            val (start, end, word) = words[i]
            var matched = false

            // 1. New Fields
            if (word.startsWith("@@") || word.startsWith("loc:")) {
                builder.addStyle(SpanStyle(color = COLOR_LOCATION), start, end)
                matched = true
            } else if (word.startsWith("url:") || (word.startsWith("[[") && word.endsWith("]]"))) {
                builder.addStyle(SpanStyle(color = COLOR_URL), start, end)
                matched = true
            } else if (word.startsWith("geo:") || word.startsWith("desc:")) {
                builder.addStyle(SpanStyle(color = COLOR_META), start, end)
                matched = true
            }

            // 2. Priority (!1 to !9 only)
            if (!matched && word.startsWith("!") && word.length > 1) {
                val p = word.substring(1).toIntOrNull()
                if (p != null && p in 1..9) {
                    val color = getTaskTextColor(p, false, isDark)
                    builder.addStyle(SpanStyle(color = color, fontWeight = FontWeight.Bold), start, end)
                    matched = true
                }
            }
            // 3. Duration (~30m)
            else if (!matched && (word.startsWith("~") || word.startsWith("est:")) && word.length > 1) {
                val valStr = if (word.startsWith("~")) word.substring(1) else word.substring(4)
                if (isValidDuration(valStr)) {
                    builder.addStyle(SpanStyle(color = COLOR_DURATION), start, end)
                    matched = true
                }
            }
            // 4. Tags
            else if (!matched && word.startsWith("#")) {
                val tagName = word.removePrefix("#")
                if (tagName.isNotEmpty()) {
                    builder.addStyle(SpanStyle(color = getTagColor(tagName), fontWeight = FontWeight.Bold), start, end)
                    matched = true
                }
            }
            // 5. Recurrence: @every X Y
            else if (!matched && (word == "@every" || word == "rec:every") && i + 2 < words.size) {
                val (_, _, amount) = words[i + 1]
                val (_, unitEnd, unit) = words[i + 2]
                if (amount.toIntOrNull() != null && isValidFreqUnit(unit)) {
                    builder.addStyle(SpanStyle(color = COLOR_RECUR), start, unitEnd)
                    i += 3
                    continue
                }
            }

            // 6. Recurrence: @daily etc
            if (!matched && (word.startsWith("@") || word.startsWith("rec:"))) {
                val valStr = if (word.startsWith("@")) word.substring(1) else word.substring(4)
                if (isValidRecurrence(valStr)) {
                    builder.addStyle(SpanStyle(color = COLOR_RECUR), start, end)
                    matched = true
                }
            }

            // 7. Dates
            if (!matched && (word.startsWith("@") || word.startsWith("^") || word.startsWith("due:") || word.startsWith("start:"))) {
                val prefixChar = word.firstOrNull() ?: ' '
                val isStart = prefixChar == '^' || word.startsWith("start:")
                val cleanWord =
                    if (isStart) {
                        word.removePrefix("start:").removePrefix("^")
                    } else {
                        word.removePrefix("due:").removePrefix("@")
                    }

                if (cleanWord == "next" && i + 1 < words.size) {
                    val (_, nextEnd, nextVal) = words[i + 1]
                    if (isValidDateUnit(nextVal)) {
                        val color = if (isStart) COLOR_START else COLOR_DUE
                        builder.addStyle(SpanStyle(color = color), start, nextEnd)
                        i += 2
                        continue
                    }
                } else if (cleanWord == "in" && i + 2 < words.size) {
                    val (_, _, amountStr) = words[i + 1]
                    val (_, unitEnd, unitStr) = words[i + 2]
                    val isNum =
                        amountStr.toIntOrNull() != null ||
                            listOf(
                                "one",
                                "two",
                                "three",
                                "four",
                                "five",
                                "six",
                                "seven",
                                "eight",
                                "nine",
                                "ten",
                            ).contains(amountStr.lowercase())
                    if (isNum && isValidDurationUnit(unitStr)) {
                        val color = if (isStart) COLOR_START else COLOR_DUE
                        builder.addStyle(SpanStyle(color = color), start, unitEnd)
                        i += 3
                        continue
                    }
                }
            }

            if (!matched) {
                if (word.startsWith("^") || word.startsWith("start:")) {
                    val valStr = if (word.startsWith("^")) word.substring(1) else word.substring(6)
                    if (isValidSmartDate(valStr)) {
                        builder.addStyle(SpanStyle(color = COLOR_START), start, end)
                        matched = true
                    }
                } else if (word.startsWith("@") || word.startsWith("due:")) {
                    val valStr = if (word.startsWith("@")) word.substring(1) else word.substring(4)
                    if (isValidSmartDate(valStr)) {
                        builder.addStyle(SpanStyle(color = COLOR_DUE), start, end)
                        matched = true
                    }
                }
            }

            i++
        }

        return TransformedText(builder.toAnnotatedString(), OffsetMapping.Identity)
    }
}
