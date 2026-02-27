// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/TaskRow.kt
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
import com.trougnouf.cfait.R
import java.time.Instant
import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneId

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
                .padding(start = 12.dp + startPadding, end = 12.dp, top = 0.5.dp, bottom = 0.5.dp)
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
                val isTrash = task.calendarHref == "local://trash"
                Text(
                    text = task.summary,
                    style = MaterialTheme.typography.bodyMedium,
                    color = textColor,
                    fontWeight = if (task.priority > 0.toUByte()) FontWeight.Medium else FontWeight.Normal,
                    textDecoration = if (task.isDone || isTrash) TextDecoration.LineThrough else null,
                    lineHeight = 18.sp,
                )

                FlowRow(
                    modifier = Modifier,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalArrangement = Arrangement.spacedBy(1.dp),
                ) {
                    if (task.description.isNotEmpty() || task.blockingUids.isNotEmpty() || task.blockingNames.isNotEmpty()) {
                        // Show the info icon if the task has a description OR if it is blocking other tasks
                        // so users can tap to see the "Blocking (Successors)" section in details.
                        NfIcon(NfIcons.INFO, size = 10.sp, color = Color.Gray, lineHeight = 10.sp)
                    }

                    if (task.relatedToUids.isNotEmpty() || incomingRelations.isNotEmpty()) {
                        val relatedUid = if (task.relatedToUids.isNotEmpty()) {
                            task.relatedToUids[0]
                        } else {
                            incomingRelations[0]
                        }
                        NfIcon(
                            getRandomRelatedIcon(task.uid, relatedUid),
                            size = 10.sp,
                            color = Color.Gray,
                            lineHeight = 10.sp
                        )
                    }

                    if (task.isBlocked) NfIcon(
                        NfIcons.BLOCKED,
                        size = 10.sp,
                        color = MaterialTheme.colorScheme.error,
                        lineHeight = 10.sp
                    )

                    if (task.hasAlarms) {
                        NfIcon(NfIcons.BELL, size = 10.sp, color = Color(0xFFFF7043), lineHeight = 10.sp)
                    }

                    // Date Display Logic
                    if (task.isDone && task.completedDateIso != null) {
                        // COMPLETED/CANCELLED DATE DISPLAY
                        val (done_icon, doneColor) = if (task.statusString == "Completed") {
                            Pair(NfIcons.CALENDAR_CHECK, Color(0xFF66BB6A)) // Greenish
                        } else { // Cancelled
                            Pair(NfIcons.CALENDAR_XMARK, MaterialTheme.colorScheme.error) // Red
                        }

                        NfIcon(done_icon, size = 10.sp, color = doneColor, lineHeight = 10.sp)

                        // Show the full datetime
                        val dateStr = try {
                            formatIsoToLocal(task.completedDateIso!!)
                        } catch (e: Exception) {
                            // Fallback: if parsing fails, safely take up to the first 16 chars
                            val safeIso = task.completedDateIso ?: ""
                            if (safeIso.length >= 16) safeIso.substring(0, 16).replace("T", " ") else safeIso
                        }
                        Text(dateStr, fontSize = 10.sp, color = doneColor, lineHeight = 10.sp)

                    } else if (task.isFutureStart && task.startDateIso != null) {
                        // Future Start Display
                        val dimColor = Color(0xFFBDBDBD) // Lighter Gray

                        // Start Icon
                        NfIcon(NfIcons.HOURGLASS_START, size = 10.sp, color = dimColor, lineHeight = 10.sp)

                        // Format Start Date
                        val startStr = if (task.isAlldayStart) {
                            task.startDateIso!!.take(10)
                        } else {
                            formatIsoToLocal(task.startDateIso!!)
                        }

                        if (task.dueDateIso != null) {
                            // Format Due Date (raw)
                            val rawDueStr = if (task.isAlldayDue) {
                                task.dueDateIso!!.take(10)
                            } else {
                                formatIsoToLocal(task.dueDateIso!!)
                            }

                            // Smart condense logic:
                            // Check if date parts (first 10 chars "YYYY-MM-DD") match
                            // and if we are not AllDay (AllDay strings are length 10, Specific are longer)
                            val displayDueStr = if (
                                startStr.length >= 10 &&
                                rawDueStr.length >= 10 &&
                                startStr.substring(0, 10) == rawDueStr.substring(0, 10) &&
                                !task.isAlldayDue
                            ) {
                                // Extract just the time "HH:MM"
                                // formatIsoToLocal returns "YYYY-MM-DD HH:MM", so drop first 11 chars
                                if (rawDueStr.length > 11) rawDueStr.substring(11) else rawDueStr
                            } else {
                                rawDueStr
                            }

                            if (startStr == rawDueStr) {
                                // Case 2: Start == Due -> Show once
                                Text(startStr, fontSize = 10.sp, color = dimColor, lineHeight = 10.sp)
                            } else {
                                // Case 1: Start != Due -> Show range
                                Text(
                                    "$startStr - $displayDueStr",
                                    fontSize = 10.sp,
                                    color = dimColor,
                                    lineHeight = 10.sp
                                )
                            }
                            // End Icon
                            NfIcon(NfIcons.HOURGLASS_END, size = 10.sp, color = dimColor, lineHeight = 10.sp)
                        } else {
                            // Case 4: Start Only
                            Text(startStr, fontSize = 10.sp, color = dimColor, lineHeight = 10.sp)
                        }

                    } else if (!task.dueDateIso.isNullOrEmpty()) {
                        // Case 3: Due Only (or Started)
                        // Note: Removed generic CALENDAR icon check to rely on the new hourglass_end

                        // --- CHANGED START ---
                        // Check for Overdue
                        val isOverdue = remember(task.dueDateIso, task.isDone, task.isAlldayDue) {
                            if (task.isDone || task.dueDateIso == null) {
                                false
                            } else {
                                try {
                                    val dueInstant = if (task.isAlldayDue) {
                                        // All-day task: "YYYY-MM-DD". Overdue if the *next day* has started.
                                        val localDate = LocalDate.parse(task.dueDateIso)
                                        localDate.plusDays(1)
                                            .atStartOfDay(ZoneId.systemDefault())
                                            .toInstant()
                                    } else {
                                        // Specific time task: "YYYY-MM-DDTHH:MM:SSZ"
                                        OffsetDateTime.parse(task.dueDateIso).toInstant()
                                    }
                                    // It's overdue if the due time is before the current time.
                                    dueInstant.isBefore(Instant.now())
                                } catch (e: Exception) {
                                    // If parsing fails for any reason, it's not overdue.
                                    false
                                }
                            }
                        }

                        val dueColor = if (isOverdue) {
                            MaterialTheme.colorScheme.error // Red
                        } else {
                            Color.Gray
                        };

                        // Format Due Date
                        val displayStr = if (task.isAlldayDue) {
                            task.dueDateIso!!.take(10)
                        } else {
                            formatIsoToLocal(task.dueDateIso!!)
                        }

                        Text(displayStr, fontSize = 10.sp, color = dueColor, lineHeight = 10.sp)
                        NfIcon(NfIcons.HOURGLASS_END, size = 10.sp, color = dueColor, lineHeight = 10.sp)
                        // --- CHANGED END ---
                    }

                    // DURATION DISPLAY
                    // Calculate Total Spent (Stored + Live)
                    var liveDurationMins by remember(task.timeSpentSeconds, task.lastStartedAt) {
                        mutableStateOf((task.timeSpentSeconds / 60u).toInt())
                    }

                    if (task.lastStartedAt != null) {
                        LaunchedEffect(task.lastStartedAt) {
                            while (true) {
                                val now = System.currentTimeMillis() / 1000
                                val start = task.lastStartedAt!!
                                val currentSession = if (now > start) now - start else 0
                                val totalSeconds = task.timeSpentSeconds.toLong() + currentSession
                                liveDurationMins = (totalSeconds / 60).toInt()
                                kotlinx.coroutines.delay(60000) // Update every minute
                            }
                        }
                    }

                    if (liveDurationMins > 0 || task.durationMins != null || task.lastStartedAt != null) {
                        // UPDATE: Pass isEstimate=false for spent, isEstimate=true for duration
                        val spentStr = if (liveDurationMins > 0 || task.lastStartedAt != null) formatDuration(
                            liveDurationMins.toUInt(),
                            isEstimate = false
                        ) else ""
                        val estStr = if (task.durationMins != null) {
                            formatDuration(task.durationMins!!, task.durationMaxMins, isEstimate = true)
                        } else ""

                        val label = when {
                            spentStr.isNotEmpty() && estStr.isNotEmpty() -> "$spentStr / $estStr"
                            spentStr.isNotEmpty() -> spentStr
                            else -> estStr
                        }

                        // Use a specific color for active tracking to draw attention
                        val durColor = if (task.lastStartedAt != null) Color(0xFF66BB6A) else Color.Gray

                        Text(label, fontSize = 10.sp, color = durColor, lineHeight = 10.sp)
                    }
                    if (task.isRecurring) NfIcon(NfIcons.REPEAT, size = 10.sp, color = Color.Gray, lineHeight = 10.sp)

                    if (task.geo != null) {
                        IconButton(
                            onClick = { uriHandler.openUri("geo:${task.geo}") },
                            modifier = Modifier.size(14.dp).padding(0.dp),
                        ) {
                            NfIcon(
                                NfIcons.MAP_LOCATION_DOT,
                                size = 10.sp,
                                color = Color(0xFF64B5F6),
                                lineHeight = 10.sp
                            )
                        }
                    }

                    // Location Text - Only show if not hidden by alias
                    if (task.location != null && task.location != hiddenLocation) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            val locationColor = Color(0xFFFFB300)
                            Text(
                                "@@",
                                fontSize = 10.sp,
                                color = locationColor,
                                fontWeight = FontWeight.Bold,
                                lineHeight = 10.sp
                            )
                            Text(task.location!!, fontSize = 10.sp, color = locationColor, lineHeight = 10.sp)
                        }
                    }

                    if (task.url != null) {
                        IconButton(
                            onClick = { uriHandler.openUri(task.url!!) },
                            modifier = Modifier.size(14.dp).padding(0.dp),
                        ) {
                            NfIcon(NfIcons.WEB_CHECK, size = 10.sp, color = Color(0xFF4FC3F7), lineHeight = 10.sp)
                        }
                    }

                    // Categories - Only show if not hidden by alias
                    task.categories.forEach { tag ->
                        if (!hiddenTags.contains(tag)) {
                            Text(
                                "#$tag",
                                fontSize = 10.sp,
                                color = getTagColor(tag, isDark),
                                modifier = Modifier.padding(end = 2.dp),
                                lineHeight = 10.sp
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
                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_edit)) },
                        onClick = {
                            expanded = false
                            onClick(task.uid)
                        },
                        leadingIcon = { NfIcon(NfIcons.EDIT, 16.sp) })

                    DropdownMenuItem(
                        text = {
                            Text(
                                if (task.statusString == "InProcess") {
                                    androidx.compose.ui.res.stringResource(R.string.menu_pause)
                                } else if (task.isPaused) {
                                    androidx.compose.ui.res.stringResource(R.string.menu_resume)
                                } else {
                                    androidx.compose.ui.res.stringResource(R.string.menu_start)
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
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_stop_reset)) },
                            onClick = {
                                expanded = false
                                onAction("stop")
                            },
                            leadingIcon = { NfIcon(NfIcons.DEBUG_STOP, 16.sp) })
                    }
                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_increase_prio)) },
                        onClick = {
                            expanded = false
                            onAction("prio_up")
                        },
                        leadingIcon = { NfIcon(NfIcons.PRIORITY_UP, 16.sp) })
                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_decrease_prio)) },
                        onClick = {
                            expanded = false
                            onAction("prio_down")
                        },
                        leadingIcon = { NfIcon(NfIcons.PRIORITY_DOWN, 16.sp) })
                    if (yankedUid == null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_yank)) },
                            onClick = {
                                expanded = false
                                onAction("yank")
                            },
                            leadingIcon = { NfIcon(NfIcons.LINK, 16.sp) })
                    }

                    DropdownMenuItem(
                        text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_create_subtask)) },
                        onClick = {
                            expanded = false
                            onAction("create_child")
                        },
                        leadingIcon = { NfIcon(NfIcons.CHILD, 16.sp) }
                    )

                    if (enabledCalendarCount > 1) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_move)) },
                            onClick = {
                                expanded = false
                                onAction("move")
                            },
                            leadingIcon = { NfIcon(NfIcons.MOVE, 16.sp) })
                    }

                    if (task.statusString != "Cancelled") {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.cancel)) },
                            onClick = {
                                expanded = false
                                onAction("cancel")
                            },
                            leadingIcon = { NfIcon(NfIcons.CROSS, 16.sp) })
                    }
                    if (task.geo != null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_location)) },
                            onClick = {
                                expanded = false
                                uriHandler.openUri("geo:${task.geo}")
                            },
                            leadingIcon = { NfIcon(NfIcons.MAP_LOCATION_DOT, 16.sp) })
                    }

                    if (task.url != null) {
                        DropdownMenuItem(
                            text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_link)) },
                            onClick = {
                                expanded = false
                                uriHandler.openUri(task.url!!)
                            },
                            leadingIcon = { NfIcon(NfIcons.WEB_CHECK, 16.sp) })
                    }

                    DropdownMenuItem(text = {
                        Text(
                            androidx.compose.ui.res.stringResource(R.string.menu_delete),
                            color = MaterialTheme.colorScheme.error
                        )
                    }, onClick = {
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

        // Render Focus Button if callback exists
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
