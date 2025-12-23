// File: android/app/src/main/java/com/cfait/ui/TaskRow.kt
package com.cfait.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.core.MobileTask

@OptIn(ExperimentalLayoutApi::class, ExperimentalFoundationApi::class)
@Composable
fun TaskRow(
    task: MobileTask,
    calColor: Color,
    isDark: Boolean,
    onToggle: () -> Unit,
    onAction: (String) -> Unit,
    onClick: (String) -> Unit,
    yankedUid: String?,
) {
    val startPadding = (task.depth.toInt() * 12).dp
    var expanded by remember { mutableStateOf(false) }
    val textColor = getTaskTextColor(task.priority.toInt(), task.isDone, isDark)
    val highlightColor = Color(0xFFffe600).copy(alpha = 0.1f)
    val containerColor = if (expanded) highlightColor else MaterialTheme.colorScheme.surface

    Card(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(start = 12.dp + startPadding, end = 12.dp, top = 1.dp, bottom = 1.dp)
                .combinedClickable(
                    onClick = { onClick(task.uid) },
                    onLongClick = { expanded = true },
                ),
        colors = CardDefaults.cardColors(containerColor = containerColor),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TaskCheckbox(task, calColor, onToggle)
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    text = task.summary,
                    style = MaterialTheme.typography.bodyMedium,
                    color = textColor,
                    fontWeight = if (task.priority > 0.toUByte()) FontWeight.Medium else FontWeight.Normal,
                    textDecoration = if (task.isDone) TextDecoration.LineThrough else null,
                    lineHeight = 18.sp,
                )

                // Render Metadata
                FlowRow(
                    modifier = Modifier.padding(top = 2.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    if (task.isBlocked) NfIcon(NfIcons.BLOCKED, 10.sp, MaterialTheme.colorScheme.error)
                    if (!task.dueDateIso.isNullOrEmpty()) {
                        NfIcon(NfIcons.CALENDAR, 10.sp, Color.Gray)
                        Text(task.dueDateIso!!.take(10), fontSize = 10.sp, color = Color.Gray)
                    }
                    if (task.durationMins != null) {
                        Text(formatDuration(task.durationMins!!), fontSize = 10.sp, color = Color.Gray)
                    }
                    if (task.isRecurring) NfIcon(NfIcons.REPEAT, 10.sp, Color.Gray)

                    // --- NEW FIELDS ---
                    if (task.location != null) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            val locationColor = Color(0xFFFFB300)
                            Text("@@", fontSize = 10.sp, color = locationColor, fontWeight = FontWeight.Bold)
                            Spacer(Modifier.width(2.dp))
                            Text(task.location!!, fontSize = 10.sp, color = locationColor)
                        }
                    }
                    if (task.url != null) {
                        // URL Icon
                        NfIcon(NfIcons.URL, 10.sp, Color(0xFF4FC3F7))
                    }
                    // ------------------

                    task.categories.forEach { tag ->
                        Text("#$tag", fontSize = 10.sp, color = getTagColor(tag), modifier = Modifier.padding(end = 2.dp))
                    }
                }
            }

            if (yankedUid != null && yankedUid != task.uid) {
                IconButton(onClick = { onAction("block") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(NfIcons.BLOCKED, 18.sp, MaterialTheme.colorScheme.secondary)
                }
                IconButton(onClick = { onAction("child") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(NfIcons.CHILD, 18.sp, MaterialTheme.colorScheme.secondary)
                }
            }

            Box {
                IconButton(onClick = { expanded = true }, modifier = Modifier.size(24.dp)) { NfIcon(NfIcons.DOTS_CIRCLE, 16.sp) }
                DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                    DropdownMenuItem(text = { Text("Edit") }, onClick = {
                        expanded = false
                        onClick(task.uid)
                    }, leadingIcon = { NfIcon(NfIcons.EDIT, 16.sp) })
                    DropdownMenuItem(
                        text = {
                            Text(
                                if (task.statusString == "InProcess") {
                                    "Pause"
                                } else if (task.isPaused) {
                                    "Resume"
                                } else {
                                    "Start"
                                },
                            )
                        },
                        onClick = {
                            expanded = false
                            onAction("playpause")
                        },
                        leadingIcon = { NfIcon(if (task.statusString == "InProcess") NfIcons.PAUSE else NfIcons.PLAY, 16.sp) },
                    )
                    if (task.statusString == "InProcess" || task.isPaused) {
                        DropdownMenuItem(text = { Text("Stop (Reset)") }, onClick = {
                            expanded = false
                            onAction("stop")
                        }, leadingIcon = { NfIcon(NfIcons.DEBUG_STOP, 16.sp) })
                    }
                    DropdownMenuItem(text = { Text("Increase prio") }, onClick = {
                        expanded = false
                        onAction("prio_up")
                    }, leadingIcon = { NfIcon(NfIcons.PRIORITY_UP, 16.sp) })
                    DropdownMenuItem(text = { Text("Decrease prio") }, onClick = {
                        expanded = false
                        onAction("prio_down")
                    }, leadingIcon = { NfIcon(NfIcons.PRIORITY_DOWN, 16.sp) })
                    if (yankedUid == null) {
                        DropdownMenuItem(text = { Text("Yank (link)") }, onClick = {
                            expanded = false
                            onAction("yank")
                        }, leadingIcon = { NfIcon(NfIcons.LINK, 16.sp) })
                    }
                    if (task.statusString != "Cancelled") {
                        DropdownMenuItem(text = { Text("Cancel") }, onClick = {
                            expanded = false
                            onAction("cancel")
                        }, leadingIcon = { NfIcon(NfIcons.CROSS, 16.sp) })
                    }
                    DropdownMenuItem(text = { Text("Delete", color = MaterialTheme.colorScheme.error) }, onClick = {
                        expanded = false
                        onAction("delete")
                    }, leadingIcon = { NfIcon(NfIcons.DELETE, 16.sp, MaterialTheme.colorScheme.error) })
                }
            }
        }
    }
}

@Composable
fun TaskCheckbox(
    task: MobileTask,
    calColor: Color,
    onClick: () -> Unit,
) {
    val isDone = task.isDone
    val status = task.statusString
    val isPaused = task.isPaused
    val bgColor =
        when {
            isDone -> Color(0xFF009900)
            status == "InProcess" -> Color(0xFF99CC99)
            isPaused -> Color(0xFFFFD54F)
            status == "Cancelled" -> Color(0xFF4D3333)
            else -> Color.Transparent
        }
    Box(
        modifier =
            Modifier
                .size(
                    20.dp,
                ).background(bgColor, RoundedCornerShape(4.dp))
                .border(1.5.dp, calColor, RoundedCornerShape(4.dp))
                .clickable {
                    onClick()
                },
        contentAlignment = Alignment.Center,
    ) {
        if (isDone) {
            NfIcon(NfIcons.CHECK, 12.sp, Color.White)
        } else if (status == "InProcess") {
            Box(Modifier.offset(y = (-2).dp)) { NfIcon(NfIcons.PLAY, 10.sp, Color.White) }
        } else if (isPaused) {
            NfIcon(NfIcons.PAUSE, 10.sp, Color.Black)
        } else if (status == "Cancelled") {
            NfIcon(NfIcons.CROSS, 12.sp, Color.White)
        }
    }
}

@Composable
fun CompactTagRow(
    name: String,
    count: Int?,
    color: Color,
    isSelected: Boolean,
    onClick: () -> Unit,
    icon: String = NfIcons.TAG,
) {
    val bg = if (isSelected) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .height(36.dp)
                .background(bg, RoundedCornerShape(4.dp))
                .clickable {
                    onClick()
                }.padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        NfIcon(icon, size = 14.sp, color = color)
        Spacer(Modifier.width(12.dp))
        Text(name, fontSize = 14.sp, modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurface)
        if (count != null) Text("$count", fontSize = 12.sp, color = Color.Gray)
    }
}
