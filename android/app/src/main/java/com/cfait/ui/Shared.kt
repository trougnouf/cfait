// File: ./android/app/src/main/java/com/cfait/ui/Shared.kt
package com.cfait.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import com.cfait.R

val NerdFont = FontFamily(Font(R.font.symbols_nerd_font))

object NfIcons {
    fun get(code: Int): String = String(Character.toChars(code))
    
    // Aligned with src/gui/icon.rs
    val CALENDAR = get(0xf073)     // 
    val TAG = get(0xf02b)          // 
    val SETTINGS = get(0xe690)     // nf-seti-settings (using SETTINGS_GEAR)
    val REFRESH = get(0xf0450)     // nf-md-refresh
    val DELETE = get(0xf1f8)       //  (TRASH)
    val CHECK = get(0xf00c)        // 
    val CROSS = get(0xf00d)        // 
    val PLAY = get(0xeb2c)         // nf-cod-play
    val PAUSE = get(0xf04c)        // 
    val REPEAT = get(0xf0b6)       // 
    val VISIBLE = get(0xea70)      // nf-cod-eye (EYE)
    val HIDDEN = get(0xeae7)       // nf-cod-eye_closed (EYE_CLOSED)
    val WRITE_TARGET = get(0xf0cfb) // nf-md-content_save_edit
    val ADD = get(0xf067)          // a plus sign, good enough for 'send'
    val BACK = get(0xf060)         //  (arrow-left)
    val PRIORITY_UP = get(0xf0603) // nf-md-priority_high
    val PRIORITY_DOWN = get(0xf0604) // nf-md-priority_low
    val EDIT = get(0xf040)         // 
    val ARROW_RIGHT = get(0xf061)  // 
    val LINK = get(0xf0c1)         // 
    val UNLINK = get(0xf127)       // 
    val INFO = get(0xf129)          // 
    val UNSYNCED = get(0xf0c2)      //  (Cloud)
    val EXPORT = get(0xeac3)        //  (Cloud Upload)
    val HELP = get(0xf0625)         // 󰘥 (Help Rhombus / Circle)
    
    // YANK ACTION ICONS
    val BLOCKED = get(0xf479)      // nf-oct-blocked
    val CHILD = get(0xf0a89)       // nf-md-account_child
    
    // Android-specific or common alternates not in gui/icon.rs
    val SEARCH = get(0xf002)        // 
    val MENU = get(0xf0c9)          // 
    val DOTS_CIRCLE = get(0xf1978)   // custom icon for menu
    val COPY = get(0xf0c5)           // for yank
}

@Composable
fun NfIcon(text: String, size: TextUnit = 24.sp, color: Color = MaterialTheme.colorScheme.onSurface) {
    Text(text = text, fontFamily = NerdFont, fontSize = size, color = color)
}

fun parseHexColor(hex: String): Color {
    return try {
        var clean = hex.removePrefix("#")
        if (clean.length > 6) { clean = clean.take(6) }
        val colorInt = android.graphics.Color.parseColor("#$clean")
        Color(colorInt)
    } catch (e: Exception) { Color.Gray }
}

fun getTagColor(tag: String): Color {
    val hash = tag.hashCode()
    val h = (kotlin.math.abs(hash) % 360).toFloat()
    return Color.hsv(h, 0.6f, 0.5f)
}

fun getTaskTextColor(prio: Int, isDone: Boolean, isDark: Boolean): Color {
    if (isDone) return Color.Gray
    return when(prio) {
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