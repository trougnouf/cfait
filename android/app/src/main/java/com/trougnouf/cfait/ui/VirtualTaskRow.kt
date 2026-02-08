package com.trougnouf.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.core.MobileTask

// Remove incorrect import: import com.trougnouf.cfait.core.NerdFont
// NerdFont is defined in com.trougnouf.cfait.ui package, so it's available directly here.

/**
 * A simple row used to render the lightweight virtual tasks injected by the Rust core
 * (expand / collapse placeholders for truncated completed/cancelled groups).
 *
 * The row renders a single Nerd Font glyph and is clickable to toggle the group state.
 */
@Composable
fun VirtualTaskRow(task: MobileTask, onClick: () -> Unit) {
    // Indent by depth * 12dp
    val startPadding = (task.depth.toInt() * 12).dp

    // Use explicit codepoints to avoid invalid \u escapes for large codepoints.
    // Nerd Font glyphs for expand / collapse (md arrow expand icons).
    val expandCodepoint = 0xF0796
    val collapseCodepoint = 0xF0799

    val iconStr = if (task.virtualType == "expand") {
        String(Character.toChars(expandCodepoint))
    } else {
        String(Character.toChars(collapseCodepoint))
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 12.dp + startPadding, end = 12.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = iconStr,
            fontFamily = NerdFont,
            fontSize = 20.sp,
            color = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f)
        )
    }
}
