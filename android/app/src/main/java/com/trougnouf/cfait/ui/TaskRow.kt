// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/TaskRow.kt
// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Composable component for rendering a single task row in the UI.
 */

 package com.trougnouf.cfait.ui

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

@Composable
fun TaskCheckbox(
    isDone: Boolean,
    status: String,
    isPaused: Boolean,
    calColor: Color,
    onClick: () -> Unit,
) {
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

 @OptIn(ExperimentalLayoutApi::class, androidx.compose.foundation.ExperimentalFoundationApi::class)
 @Composable
 fun TaskRow(
     task: MobileTask,
     calColor: Color,
     onToggle: () -> Unit,
     onAction: (String) -> Unit,
     onClick: (String) -> Unit,
     yankedUid: String?,
     enabledCalendarCount: Int,
     isHighlighted: Boolean = false,
     isCollapsed: Boolean = false,
     onToggleCollapse: () -> Unit = {}
 ) {
     val visuals = task.visuals
     val startPadding = (visuals.depth.toInt() * 12).dp
     var expanded by remember { mutableStateOf(false) }

     val titleColor = Color(android.graphics.Color.parseColor(visuals.titleColorHex))
     val dateColor = Color(android.graphics.Color.parseColor(visuals.dateColorHex))
     val durColor = Color(android.graphics.Color.parseColor(visuals.durationColorHex))

     val highlightColor = Color(0xFFffe600).copy(alpha = 0.1f)
     val containerColor = if (isHighlighted || expanded) highlightColor else MaterialTheme.colorScheme.surface
     val uriHandler = LocalUriHandler.current

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

             TaskCheckbox(
                 isDone = visuals.isDone,
                 status = visuals.statusString,
                 isPaused = visuals.isPaused,
                 calColor = calColor,
                 onClick = onToggle
             )
             Spacer(Modifier.width(8.dp))

             Column(Modifier.weight(1f)) {
                 Text(
                     text = visuals.summary,
                     style = MaterialTheme.typography.bodyMedium,
                     color = titleColor,
                     fontWeight = if (task.priority > 0.toUByte()) FontWeight.Medium else FontWeight.Normal,
                     textDecoration = if (visuals.isDone) TextDecoration.LineThrough else null,
                     lineHeight = 18.sp,
                 )

                 FlowRow(
                     horizontalArrangement = Arrangement.spacedBy(6.dp),
                     verticalArrangement = Arrangement.spacedBy(2.dp),
                 ) {
                     if (visuals.hasNotesOrDeps) {
                         NfIcon(NfIcons.INFO, size = 10.sp, color = Color.Gray, lineHeight = 10.sp)
                     }

                     if (visuals.isBlocked) {
                         NfIcon(NfIcons.BLOCKED, size = 10.sp, color = MaterialTheme.colorScheme.error, lineHeight = 10.sp)
                     }

                     if (visuals.hasActiveAlarm) {
                         NfIcon(NfIcons.BELL, size = 10.sp, color = Color(0xFFFF7043), lineHeight = 10.sp)
                     }

                     visuals.dateBadge?.let { badge ->
                         Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                             Text(visuals.dateIcon, fontFamily = NerdFont, fontSize = 10.sp, color = dateColor, lineHeight = 10.sp)
                             Text(badge, fontSize = 10.sp, color = dateColor, lineHeight = 10.sp)
                         }
                     }

                     visuals.durationBadge?.let { badge ->
                         Text(badge, fontSize = 10.sp, color = durColor, lineHeight = 10.sp)
                     }

                     visuals.locationBadge?.let { loc ->
                         Text("@@$loc", fontSize = 10.sp, color = Color(0xFFFFB300), lineHeight = 10.sp)
                     }

                     visuals.tags.forEach { tag ->
                         val bg = Color(android.graphics.Color.parseColor(tag.bgColorHex))
                         val text = Color(android.graphics.Color.parseColor(tag.textColorHex))
                         Box(modifier = Modifier.background(bg, RoundedCornerShape(4.dp)).padding(horizontal = 4.dp, vertical = 2.dp)) {
                             Text("#${tag.name}", color = text, fontSize = 10.sp, lineHeight = 10.sp)
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

             if (visuals.hasSubtasks || isCollapsed) {
                 val iconChar = if (isCollapsed) NfIcons.FAMILY_TREE else NfIcons.TREE_FA
                 val iconColor = if (isCollapsed) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.6f)

                 IconButton(onClick = onToggleCollapse, modifier = Modifier.size(24.dp)) {
                     NfIcon(iconChar, 16.sp, iconColor)
                 }
             }

             Box {
                 IconButton(onClick = { expanded = true }, modifier = Modifier.size(24.dp)) {
                     NfIcon(NfIcons.DOTS_CIRCLE, 16.sp)
                 }
                 DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                     DropdownMenuItem(
                         text = { Text(androidx.compose.ui.res.stringResource(R.string.edit)) },
                         onClick = { expanded = false; onClick(task.uid) },
                         leadingIcon = { NfIcon(NfIcons.EDIT, 16.sp) })

                     DropdownMenuItem(
                         text = {
                             Text(
                                 if (task.statusString == "InProcess") androidx.compose.ui.res.stringResource(R.string.pause)
                                 else if (task.isPaused) androidx.compose.ui.res.stringResource(R.string.menu_resume)
                                 else androidx.compose.ui.res.stringResource(R.string.start)
                             )
                         },
                         onClick = { expanded = false; onAction("playpause") },
                         leadingIcon = { NfIcon(if (task.statusString == "InProcess") NfIcons.PAUSE else NfIcons.PLAY, 16.sp) }
                     )

                     if (task.statusString == "InProcess" || task.isPaused) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.stop_reset)) },
                             onClick = { expanded = false; onAction("stop") },
                             leadingIcon = { NfIcon(NfIcons.DEBUG_STOP, 16.sp) })
                     }

                     DropdownMenuItem(
                         text = { Text(androidx.compose.ui.res.stringResource(R.string.increase_priority)) },
                         onClick = { expanded = false; onAction("prio_up") },
                         leadingIcon = { NfIcon(NfIcons.PRIORITY_UP, 16.sp) })

                     DropdownMenuItem(
                         text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_decrease_prio)) },
                         onClick = { expanded = false; onAction("prio_down") },
                         leadingIcon = { NfIcon(NfIcons.PRIORITY_DOWN, 16.sp) })

                     if (yankedUid == null) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_yank)) },
                             onClick = { expanded = false; onAction("yank") },
                             leadingIcon = { NfIcon(NfIcons.LINK, 16.sp) })
                     }

                     DropdownMenuItem(
                         text = { Text(androidx.compose.ui.res.stringResource(R.string.create_subtask)) },
                         onClick = { expanded = false; onAction("create_child") },
                         leadingIcon = { NfIcon(NfIcons.CHILD, 16.sp) }
                     )

                     if (task.parentUid != null) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.promote_remove_parent)) },
                             onClick = { expanded = false; onAction("promote") },
                             leadingIcon = { NfIcon(NfIcons.ELEVATOR_UP, 16.sp) }
                         )
                     }

                     val duplicateLabel = if (task.hasSubtasks) androidx.compose.ui.res.stringResource(R.string.duplicate_task)
                     else androidx.compose.ui.res.stringResource(R.string.duplicate_single_task)

                     DropdownMenuItem(
                         text = { Text(duplicateLabel) },
                         onClick = { expanded = false; onAction("duplicate") },
                         leadingIcon = { NfIcon(NfIcons.CLONE, 16.sp) }
                     )

                     if (enabledCalendarCount > 1) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_move)) },
                             onClick = { expanded = false; onAction("move") },
                             leadingIcon = { NfIcon(NfIcons.MOVE, 16.sp) })
                     }

                     if (task.statusString != "Cancelled") {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.cancel)) },
                             onClick = { expanded = false; onAction("cancel") },
                             leadingIcon = { NfIcon(NfIcons.CROSS, 16.sp) })
                     }

                     if (task.geo != null) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_location)) },
                             onClick = { expanded = false; uriHandler.openUri("geo:${task.geo}") },
                             leadingIcon = { NfIcon(NfIcons.MAP_LOCATION_DOT, 16.sp) })
                     }

                     if (task.treeLocationCount.toInt() > 1) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.action_open_locations)) },
                             onClick = { expanded = false; onAction("open_locations_gpx") },
                             leadingIcon = { NfIcon(NfIcons.MAP_MARKER_MULTIPLE, 16.sp) })
                     }

                     if (task.url != null) {
                         DropdownMenuItem(
                             text = { Text(androidx.compose.ui.res.stringResource(R.string.menu_open_link)) },
                             onClick = { expanded = false; uriHandler.openUri(task.url!!) },
                             leadingIcon = { NfIcon(NfIcons.WEB_CHECK, 16.sp) })
                     }

                     DropdownMenuItem(text = { Text(androidx.compose.ui.res.stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                         onClick = { expanded = false; onAction("delete") },
                         leadingIcon = { NfIcon(NfIcons.DELETE, 16.sp, MaterialTheme.colorScheme.error) })

                     if (task.hasSubtasks) {
                         DropdownMenuItem(text = { Text(androidx.compose.ui.res.stringResource(R.string.delete_task_tree), color = MaterialTheme.colorScheme.error) },
                             onClick = { expanded = false; onAction("delete_tree") },
                             leadingIcon = { NfIcon(NfIcons.DELETE, 16.sp, MaterialTheme.colorScheme.error) })
                     }
                 }
             }
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
     onFocus: (() -> Unit)? = null
 ) {
     val bg = if (isSelected) color.copy(alpha = 0.15f) else Color.Transparent

     Row(
         modifier =
             Modifier
                 .fillMaxWidth()
                 .height(36.dp)
                 .background(bg, RoundedCornerShape(4.dp))
                 .clickable { onClick() }
                 .padding(horizontal = 12.dp),
         verticalAlignment = Alignment.CenterVertically,
     ) {
         NfIcon(icon, size = 14.sp, color = color)
         Spacer(Modifier.width(12.dp))
         Text(name, fontSize = 14.sp, modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurface)
         if (count != null) Text("$count", fontSize = 12.sp, color = Color.Gray)

         if (onFocus != null) {
             Spacer(Modifier.width(8.dp))
             IconButton(onClick = onFocus, modifier = Modifier.size(24.dp)) {
                 NfIcon(NfIcons.ARROW_RIGHT, 14.sp)
             }
         }
     }
 }
