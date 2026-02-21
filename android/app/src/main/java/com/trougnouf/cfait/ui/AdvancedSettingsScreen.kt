// File: ./android/app/src/main/java/com/trougnouf/cfait/ui/AdvancedSettingsScreen.kt
package com.trougnouf.cfait.ui

import android.content.Intent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.trougnouf.cfait.core.CfaitMobile
import kotlinx.coroutines.launch
import java.io.File

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AdvancedSettingsScreen(
    api: CfaitMobile,
    maxDoneRoots: String,
    maxDoneSubtasks: String,
    trashRetention: String,
    deleteEventsOnCompletion: Boolean,
    onMaxDoneRootsChange: (String) -> Unit,
    onMaxDoneSubtasksChange: (String) -> Unit,
    onTrashRetentionChange: (String) -> Unit,
    onDeleteEventsChange: (Boolean) -> Unit,
    onBack: () -> Unit
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var debugStatus by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Advanced Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                }
            )
        }
    ) { padding ->
        val scrollState = rememberScrollState()
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize()
                .verticalScroll(scrollState)
        ) {
            // Display Limits Section
            Text(
                "Display Limits",
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )

            OutlinedTextField(
                value = maxDoneRoots,
                onValueChange = onMaxDoneRootsChange,
                label = { Text("Max completed tasks (Root)") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                "How many completed tasks to show in the main list before hiding them behind a 'Show More' button.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            OutlinedTextField(
                value = maxDoneSubtasks,
                onValueChange = onMaxDoneSubtasksChange,
                label = { Text("Max completed subtasks") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                "How many completed subtasks to show inside a parent task before truncating.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Calendar Integration Section
            Text(
                "Calendar Integration",
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(
                    checked = deleteEventsOnCompletion,
                    onCheckedChange = onDeleteEventsChange
                )
                Spacer(Modifier.width(8.dp))
                Text("Delete calendar events when tasks are completed")
            }
            Text(
                "Regardless, events are always deleted when tasks are deleted.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Data Management Section
            Text(
                "Data Management",
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            OutlinedTextField(
                value = trashRetention,
                onValueChange = onTrashRetentionChange,
                label = { Text("Trash Retention (days)") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                "Keep deleted items in local trash for this many days. Set to 0 to delete immediately.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 24.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Debug Section (Moved from SettingsScreen)
            Text(
                "Debug",
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Text(
                "Export all app data (config, cache, journals) for debugging. Credentials will be redacted.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Button(
                onClick = {
                    scope.launch {
                        try {
                            debugStatus = "Exporting data..."
                            val zipPath = api.createDebugExport()
                            val sourceFile = File(zipPath)
                            val destFile = File(context.cacheDir, "cfait_debug_export.zip")

                            sourceFile.inputStream().use { input ->
                                destFile.outputStream().use { output ->
                                    input.copyTo(output)
                                }
                            }

                            val uri = FileProvider.getUriForFile(
                                context,
                                "${context.packageName}.fileprovider",
                                destFile
                            )

                            val intent = Intent(Intent.ACTION_SEND).apply {
                                type = "application/zip"
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }

                            val shareIntent = Intent.createChooser(intent, "Export Debug Data")
                            context.startActivity(shareIntent)
                            debugStatus = "Export ready"
                        } catch (e: Exception) {
                            debugStatus = "Export failed: ${e.message}"
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer
                )
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    NfIcon(NfIcons.ARCHIVE_ARROW_UP, 16.sp)
                    Spacer(Modifier.width(8.dp))
                    Text("Export Debug ZIP")
                }
            }

            if (debugStatus.isNotEmpty()) {
                Text(
                    text = debugStatus,
                    color = if (debugStatus.startsWith("Export failed")) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(top = 8.dp),
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}
