// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.MobileTaskSummary

/**
 * A simple row used to render the lightweight virtual tasks injected by the Rust core
 * (expand / collapse placeholders for truncated completed/cancelled groups).
 *
 * The row renders an icon with descriptive text and is clickable to toggle the group state.
 */
@Composable
fun VirtualTaskRow(task: MobileTaskSummary, onClick: () -> Unit) {
    // Indent by depth * 12dp
    val startPadding = (task.depth.toInt() * 12).dp

    val isExpand = task.uid.startsWith("virtual-expand-")
    val iconStr = if (isExpand) NfIcons.ARROW_EXPAND_DOWN else NfIcons.ARROW_EXPAND_UP

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 12.dp + startPadding, end = 12.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = iconStr,
            fontFamily = NerdFont,
            fontSize = 20.sp,
            color = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f)
        )
        Text(
            text = if (isExpand) stringResource(R.string.expand_completed_tasks) else stringResource(R.string.collapse_completed_tasks),
            fontSize = 12.sp,
            color = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f),
            modifier = Modifier.padding(start = 8.dp)
        )
    }
}
