// File: ./android/app/src/main/java/com/cfait/ui/TaskDetailScreen.kt
package com.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cfait.core.CfaitMobile
import com.cfait.core.MobileCalendar
import com.cfait.core.MobileTask
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaskDetailScreen(
    api: CfaitMobile, 
    uid: String, 
    calendars: List<MobileCalendar>, 
    onBack: () -> Unit,
    onSave: (String, String) -> Unit
) {
    var task by remember { mutableStateOf<MobileTask?>(null) }
    val scope = rememberCoroutineScope()
    var smartInput by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var showMoveDialog by remember { mutableStateOf(false) }

    fun reload() {
        scope.launch {
            val all = api.getViewTasks(null, "")
            task = all.find { it.uid == uid }
            task?.let { smartInput = it.smartString; description = it.description }
        }
    }

    LaunchedEffect(uid) { reload() }

    if (task == null) { Box(Modifier.fillMaxSize()) { CircularProgressIndicator(Modifier.align(Alignment.Center)) }; return }

    if (showMoveDialog) {
        AlertDialog(
            onDismissRequest = { showMoveDialog = false },
            title = { Text("Move to calendar") },
            text = {
                LazyColumn {
                    items(calendars) { cal ->
                        if (cal.href != task!!.calendarHref) {
                            TextButton(onClick = { scope.launch { api.moveTask(uid, cal.href); showMoveDialog = false; onBack() } }, modifier = Modifier.fillMaxWidth()) { Text(cal.name) }
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { showMoveDialog = false }) { Text("Cancel") } }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Edit task") },
                navigationIcon = { IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) } },
                actions = {
                    TextButton(onClick = { showMoveDialog = true }) { Text("Move") }
                    TextButton(
                        onClick = {
                            // Optimistic Save:
                            // We delegate the actual async work to the parent (MainActivity)
                            // so we can leave this screen immediately without killing the save process.
                            onSave(smartInput, description)
                        }
                    ) { Text("Save") }
                }
            )
        }
    ) { p ->
        Column(modifier = Modifier.padding(p).padding(16.dp)) {
            OutlinedTextField(value = smartInput, onValueChange = { smartInput = it }, label = { Text("Task (smart syntax)") }, modifier = Modifier.fillMaxWidth())
            Text("Use !1, @date, #tag, ~duration", style = MaterialTheme.typography.bodySmall, color = androidx.compose.ui.graphics.Color.Gray, modifier = Modifier.padding(start = 4.dp, bottom = 16.dp))
            
            if (task!!.blockedByNames.isNotEmpty()) {
                Text("Blocked by:", color = MaterialTheme.colorScheme.error, fontWeight = FontWeight.Bold, fontSize = 14.sp)
                
                val blockedPairs = task!!.blockedByNames.zip(task!!.blockedByUids)
                
                blockedPairs.forEach { (name, blockerUid) ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically, 
                        modifier = Modifier
                            .padding(vertical = 2.dp)
                            .clickable {
                                scope.launch {
                                    api.removeDependency(task!!.uid, blockerUid)
                                    reload()
                                }
                            }
                    ) {
                        NfIcon(NfIcons.CROSS, 12.sp, MaterialTheme.colorScheme.error)
                        Spacer(Modifier.width(8.dp))
                        NfIcon(NfIcons.BLOCKED, 12.sp, androidx.compose.ui.graphics.Color.Gray)
                        Spacer(Modifier.width(4.dp))
                        Text(name, fontSize = 14.sp)
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
            }

            OutlinedTextField(
                value = description, 
                onValueChange = { description = it }, 
                label = { Text("Description") }, 
                modifier = Modifier.fillMaxWidth().weight(1f), 
                textStyle = TextStyle(textAlign = androidx.compose.ui.text.style.TextAlign.Start)
            )
        }
    }
}
