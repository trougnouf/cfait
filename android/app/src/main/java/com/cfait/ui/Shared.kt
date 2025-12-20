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

    val CALENDAR = get(0xf073) // 
    val TAG = get(0xf02b) // 
    val SETTINGS = get(0xe690) // nf-seti-settings
    val REFRESH = get(0xf0450) // nf-md-refresh
    val SYNC_ALERT = get(0xf04e7) // nf-md-sync_alert (Exclamation arrow)
    val SYNC_OFF = get(0xf04e8) // nf-md-sync_off (Slash through arrows)
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
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val builder = AnnotatedString.Builder(raw)

        // Tokenize by splitting on whitespace but preserving indices
        // We will do a manual pass similar to Rust implementation
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

            // 1. Priority
            if (word.startsWith("!") && word.length > 1 && word.substring(1).all { it.isDigit() }) {
                builder.addStyle(SpanStyle(color = Color(0xFFFF4444), fontWeight = FontWeight.Bold), start, end)
                matched = true
            }
            // 2. Duration
            else if ((word.startsWith("~") || word.startsWith("est:")) && word.length > 1) {
                builder.addStyle(SpanStyle(color = Color.Gray), start, end)
                matched = true
            }
            // 3. Tags
            else if (word.startsWith("#")) {
                val tagName = word.removePrefix("#")
                builder.addStyle(SpanStyle(color = getTagColor(tagName), fontWeight = FontWeight.Bold), start, end)
                matched = true
            }
            // 4. Recurrence: @every X Y
            else if ((word == "@every" || word == "rec:every") && i + 2 < words.size) {
                val (_, _, amount) = words[i + 1]
                val (_, unitEnd, unit) = words[i + 2]
                // Simple heuristic check
                if (amount.toIntOrNull() != null) {
                    builder.addStyle(SpanStyle(color = Color(0xFF64B5F6)), start, unitEnd)
                    i += 3
                    continue
                }
            }

            // 5. Recurrence: @daily etc
            if (!matched && (word.startsWith("@") || word.startsWith("rec:"))) {
                if (word.contains("daily") || word.contains("weekly") || word.contains("monthly") || word.contains("yearly")) {
                    builder.addStyle(SpanStyle(color = Color(0xFF64B5F6)), start, end)
                    matched = true
                }
            }

            // 6. Dates Multi-word: @next week
            if (!matched && (word == "@next" || word == "^next" || word == "due:next" || word == "start:next") && i + 1 < words.size) {
                val (_, nextEnd, nextVal) = words[i + 1]
                val isStart = word.startsWith("^") || word.startsWith("start:")
                val color = if (isStart) Color(0xFF00BCD4) else Color(0xFF2196F3)

                builder.addStyle(SpanStyle(color = color), start, nextEnd)
                i += 2
                continue
            }

            // 7. Dates Single-word
            if (!matched) {
                if (word.startsWith("^") || word.startsWith("start:")) {
                    builder.addStyle(SpanStyle(color = Color(0xFF00BCD4)), start, end)
                } else if (word.startsWith("@") || word.startsWith("due:")) {
                    builder.addStyle(SpanStyle(color = Color(0xFF2196F3)), start, end)
                }
            }

            i++
        }

        return TransformedText(builder.toAnnotatedString(), OffsetMapping.Identity)
    }
}
