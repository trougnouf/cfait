// Shared Compose UI components and syntax highlighting logic.
// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/Shared.kt
package com.trougnouf.cfait.ui

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
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileSyntaxType
import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

val NerdFont = FontFamily(Font(R.font.symbols_nerd_font))

object NfIcons {
    fun get(code: Int): String = String(Character.toChars(code))

    val CALENDARS_VIEW = get(0xf00f2)
    val TAGS_VIEW = get(0xf04fb)
    val LOCATION = get(0xef4b)
    val MAP_PIN = get(0xf276)
    val MAP = get(0xf279) // nf-fa-map (Filled)
    val MAP_O = get(0xf278) // nf-fa-map_o (Outline)
    val URL = get(0xf0c1)
    val GEO = get(0xf041)
    val CALENDAR = get(0xf073)
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
    val ARROW_UP = get(0xf062)
    val ARROW_DOWN = get(0xf063)
    val ARROW_LEFT = get(0xf060)
    val ARROW_RIGHT = get(0xf061)
    val GONDOLA = get(0xf0686)
    val ROCKET_OUTLINE = get(0xf13af)
    val ELEVATOR_UP = get(0xf12c1)
    val ESCALATOR_UP = get(0xf12bf)
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
    val BELL = get(0xf0f3)
    val HOURGLASS_START = get(0xf251)
    val HOURGLASS_END = get(0xf253)

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

    val RELATED_FEMALE_FEMALE = get(0xf0a5a)
    val RELATED_MALE_MALE = get(0xf0a5e)
    val RELATED_MALE_FEMALE = get(0xf02e8)
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

        // Fix: Use Unspecified so Text() components use the active theme's onSurface color automatically
        else -> Color.Unspecified
    }
}

fun formatDuration(
    minMinutes: UInt,
    maxMinutes: UInt? = null,
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

    return if (maxMinutes != null && maxMinutes > minMinutes) {
        val maxStr = fmt(maxMinutes.toInt())
        "~$minStr-$maxStr"
    } else {
        "~$minStr"
    }
}

fun formatIsoToLocal(isoString: String): String {
    return try {
        val instant = OffsetDateTime.parse(isoString).toInstant()
        val zone = ZoneId.systemDefault()
        val formatter = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm")
        instant.atZone(zone).format(formatter)
    } catch (e: Exception) {
        // Fallback for malformed strings or unexpected formats
        isoString.take(16).replace("T", " ")
    }
}

class SmartSyntaxTransformation(
    val api: CfaitMobile,
    val isDark: Boolean,
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
            val tokens = api.parseSmartString(raw)

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
                            getTagColor(tagName)
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

                        else -> {
                            null
                        }
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
