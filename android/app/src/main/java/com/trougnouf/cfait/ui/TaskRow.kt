// Compose UI component for rendering a single task row.
package com.trougnouf.cfait.ui

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
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.core.MobileTask

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
    enabledCalendarCount: Int,
    // New parameters for inheritance hiding
    parentCategories: List<String> = emptyList(),
    parentLocation: String? = null,
    aliasMap: Map<String, List<String>> = emptyMap(),
    isHighlighted: Boolean = false,
    incomingRelations: List<String> = emptyList()
) {
    val startPadding = (task.depth.toInt() * 12).dp
    var expanded by remember { mutableStateOf(false) }
    val textColor = getTaskTextColor(task.priority.toInt(), task.isDone, isDark)
    val highlightColor = Color(0xFFffe600).copy(alpha = 0.1f)
    val containerColor = if (isHighlighted || expanded) highlightColor else MaterialTheme.colorScheme.surface
    val uriHandler = LocalUriHandler.current

    // --- ALIAS SHADOWING LOGIC ---
    // Calculate what to hide based on THIS task's own data + the alias map.
    // 1. Start with parent inheritance
    val hiddenTags = parentCategories.toMutableSet()
    var hiddenLocation = parentLocation

    fun processExpansions(targets: List<String>) {
        targets.forEach { target ->
            when {
                target.startsWith("#") -> {
                    hiddenTags.add(target.removePrefix("#").replace("\"", "").trim())
                }

                target.startsWith("@@") -> {
                    hiddenLocation = target.removePrefix("@@").replace("\"", "").trim()
                }

                target.lowercase().startsWith("loc:") -> {
                    hiddenLocation = target.substring(4).replace("\"", "").trim()
                }
            }
        }
    }

    // Check Categories for Alias triggers
    task.categories.forEach { cat ->
        // Direct match
        aliasMap[cat]?.let { processExpansions(it) }

        // Hierarchy match (e.g. #work:project -> #work)
        var search = cat
        while (search.contains(':')) {
            val idx = search.lastIndexOf(':')
            search = search.substring(0, idx)
            aliasMap[search]?.let { processExpansions(it) }
        }
    }

    // Check Location for Alias triggers
    task.location?.let { loc ->
        val key = "@@$loc"
        aliasMap[key]?.let { processExpansions(it) }

        // Hierarchy match for location
        var search = key
        while (search.contains(':')) {
            val idx = search.lastIndexOf(':')
            if (idx < 2) break // Don't split @@
            search = search.substring(0, idx)
            aliasMap[search]?.let { processExpansions(it) }
        }
    }
    // ----------------------------

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

                FlowRow(
                    modifier = Modifier.padding(top = 2.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    if (task.description.isNotEmpty()) {
                        NfIcon(NfIcons.INFO, 10.sp, Color.Gray)
                    }

                    if (task.relatedToUids.isNotEmpty() || incomingRelations.isNotEmpty()) {
                        val relatedUid = if (task.relatedToUids.isNotEmpty()) {
                            task.relatedToUids[0]
                        } else {
                            incomingRelations[0]
                        }
                        NfIcon(getRandomRelatedIcon(task.uid, relatedUid), 10.sp, Color.Gray)
                    }

                    if (task.isBlocked) NfIcon(NfIcons.BLOCKED, 10.sp, MaterialTheme.colorScheme.error)

                    if (task.hasAlarms) {
                        NfIcon(NfIcons.BELL, 10.sp, Color(0xFFFF7043))
                    }

                    // Date Display Logic
                    if (task.isFutureStart && task.startDateIso != null) {
                        // Future Start Display
                        val dimColor = Color(0xFFBDBDBD) // Lighter Gray
                        NfIcon(NfIcons.HOURGLASS_START, 10.sp, dimColor)

                        var dateStr = if (task.isAlldayStart) {
                            task.startDateIso!!.take(10)
                        } else {
                            task.startDateIso!!.take(16).replace("T", " ")
                        }

                        if (task.dueDateIso != null) {
                            val dueStr = if (task.isAlldayDue) {
                                task.dueDateIso!!.take(10)
                            } else {
                                task.dueDateIso!!.take(16).replace("T", " ")
                            }
                            dateStr += " - $dueStr"
                        }

                        Text(dateStr, fontSize = 10.sp, color = dimColor)
                    } else if (!task.dueDateIso.isNullOrEmpty()) {
                        // Standard Due Date Display
                        if (!task.hasAlarms) {
                            NfIcon(NfIcons.CALENDAR, 10.sp, Color.Gray)
                        }
                        // Format: If all-day show YYYY-MM-DD, else show YYYY-MM-DD HH:MM
                        val displayStr = if (task.isAlldayDue) {
                            task.dueDateIso!!.take(10)
                        } else {
                            task.dueDateIso!!.take(16).replace("T", " ")
                        }
                        Text(displayStr, fontSize = 10.sp, color = Color.Gray)
                    }

                    if (task.durationMins != null) {
                        Text(formatDuration(task.durationMins!!, task.durationMaxMins), fontSize = 10.sp, color = Color.Gray)
                    }
                    if (task.isRecurring) NfIcon(NfIcons.REPEAT, 10.sp, Color.Gray)

                    if (task.geo != null) {
                        IconButton(
                            onClick = { uriHandler.openUri("geo:${task.geo}") },
                            modifier = Modifier.size(16.dp).padding(0.dp),
                        ) {
                            NfIcon(NfIcons.MAP_LOCATION_DOT, 10.sp, Color(0xFF64B5F6))
                        }
                    }

                    // Location Text - Only show if not hidden by alias
                    if (task.location != null && task.location != hiddenLocation) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            val locationColor = Color(0xFFFFB300)
                            Text("@@", fontSize = 10.sp, color = locationColor, fontWeight = FontWeight.Bold)
                            Text(task.location!!, fontSize = 10.sp, color = locationColor)
                        }
                    }

                    if (task.url != null) {
                        IconButton(
                            onClick = { uriHandler.openUri(task.url!!) },
                            modifier = Modifier.size(16.dp).padding(0.dp),
                        ) {
                            NfIcon(NfIcons.WEB_CHECK, 10.sp, Color(0xFF4FC3F7))
                        }
                    }

                    // Categories - Only show if not hidden by alias
                    task.categories.forEach { tag ->
                        if (!hiddenTags.contains(tag)) {
                            Text(
                                "#$tag",
                                fontSize = 10.sp,
                                color = getTagColor(tag),
                                modifier = Modifier.padding(end = 2.dp)
                            )
                        }
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
                IconButton(onClick = { onAction("related") }, modifier = Modifier.size(32.dp)) {
                    NfIcon(getRandomRelatedIcon(task.uid, yankedUid), 18.sp, MaterialTheme.colorScheme.secondary)
                }
            }

            Box {
                IconButton(onClick = { expanded = true }, modifier = Modifier.size(24.dp)) {
                    NfIcon(
                        NfIcons.DOTS_CIRCLE,
                        16.sp
                    )
                }
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
                        leadingIcon = {
                            NfIcon(
                                if (task.statusString == "InProcess") NfIcons.PAUSE else NfIcons.PLAY,
                                16.sp
                            )
                        },
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

                    DropdownMenuItem(
                        text = { Text("Create subtask") },
                        onClick = {
                            expanded = false
                            onAction("create_child")
                        },
                        leadingIcon = { NfIcon(NfIcons.CHILD, 16.sp) }
                    )

                    if (enabledCalendarCount > 1) {
                        DropdownMenuItem(text = { Text("Move") }, onClick = {
                            expanded = false
                            onAction("move")
                        }, leadingIcon = { NfIcon(NfIcons.MOVE, 16.sp) })
                    }

                    if (task.statusString != "Cancelled") {
                        DropdownMenuItem(text = { Text("Cancel") }, onClick = {
                            expanded = false
                            onAction("cancel")
                        }, leadingIcon = { NfIcon(NfIcons.CROSS, 16.sp) })
                    }
                    if (task.geo != null) {
                        DropdownMenuItem(text = { Text("Open location") }, onClick = {
                            expanded = false
                            uriHandler.openUri("geo:${task.geo}")
                        }, leadingIcon = { NfIcon(NfIcons.MAP_LOCATION_DOT, 16.sp) })
                    }

                    if (task.url != null) {
                        DropdownMenuItem(text = { Text("Open link") }, onClick = {
                            expanded = false
                            uriHandler.openUri(task.url!!)
                        }, leadingIcon = { NfIcon(NfIcons.WEB_CHECK, 16.sp) })
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

// ... TaskCheckbox and CompactTagRow remain unchanged ...
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
            status == "Cancelled" -> Color(0xFF4D3333)
            status == "InProcess" -> Color(0xFF99CC99)
            isPaused -> Color(0xFFFFD54F)
            isDone -> Color(0xFF009900)
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
        if (status == "Cancelled") {
            Box(Modifier.offset(y = (-1).dp)) { NfIcon(NfIcons.CROSS, 12.sp, Color.White) }
        } else if (status == "InProcess") {
            Box(Modifier.offset(y = (-2).dp)) { NfIcon(NfIcons.PLAY, 10.sp, Color.White) }
        } else if (isPaused) {
            NfIcon(NfIcons.PAUSE, 10.sp, Color.Black)
        } else if (isDone) {
            NfIcon(NfIcons.CHECK, 12.sp, Color.White)
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
    onFocus: (() -> Unit)? = null // NEW Parameter
) {
    // UPDATED: Use the specific tag color with low opacity for the background,
    // matching the Desktop GUI's visual style.
    val bg = if (isSelected) color.copy(alpha = 0.15f) else Color.Transparent

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

        // NEW: Render Focus Button if callback exists
        if (onFocus != null) {
            Spacer(Modifier.width(8.dp))
            IconButton(
                onClick = onFocus,
                modifier = Modifier.size(24.dp)
            ) {
                NfIcon(NfIcons.ARROW_RIGHT, 14.sp)
            }
        }
    }
}
