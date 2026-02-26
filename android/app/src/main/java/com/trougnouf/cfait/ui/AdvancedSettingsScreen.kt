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
import androidx.compose.ui.res.stringResource
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
    var debugIsError by remember { mutableStateOf(false) }

    // Pre-resolve strings that will be referenced from non-composable contexts (eg. inside coroutine)
    val exportExporting = stringResource(R.string.export_debug_status_exporting)
    val exportReady = stringResource(R.string.export_debug_status_ready)
    val exportFailedTemplate = stringResource(R.string.export_debug_status_failed)
    val exportShareTitle = stringResource(R.string.export_debug_share_title)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.advanced_settings_title)) },
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
                text = stringResource(R.string.display_limits),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )

            OutlinedTextField(
                value = maxDoneRoots,
                onValueChange = onMaxDoneRootsChange,
                label = { Text(stringResource(R.string.max_completed_tasks_root)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                stringResource(R.string.max_completed_tasks_root_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            OutlinedTextField(
                value = maxDoneSubtasks,
                onValueChange = onMaxDoneSubtasksChange,
                label = { Text(stringResource(R.string.max_completed_subtasks)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                stringResource(R.string.max_completed_subtasks_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Calendar Integration Section
            Text(
                stringResource(R.string.calendar_integration),
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
                Text(stringResource(R.string.delete_calendar_events_on_completion_label))
            }
            Text(
                stringResource(R.string.events_deleted_on_task_delete),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 16.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Data Management Section
            Text(
                stringResource(R.string.data_management),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            OutlinedTextField(
                value = trashRetention,
                onValueChange = onTrashRetentionChange,
                label = { Text(stringResource(R.string.trash_retention_days_label)) },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                singleLine = true
            )
            Text(
                stringResource(R.string.trash_retention_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 24.dp)
            )

            HorizontalDivider(Modifier.padding(vertical = 16.dp))

            // Debug Section (Moved from SettingsScreen)
            Text(
                stringResource(R.string.debug),
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Text(
                stringResource(R.string.debug_export_explain),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 16.dp)
            )
            Button(
                onClick = {
                    scope.launch {
                        try {
                            debugIsError = false
                            debugStatus = exportExporting
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

                            val shareIntent = Intent.createChooser(intent, exportShareTitle)
                            context.startActivity(shareIntent)
                            debugIsError = false
                            debugStatus = exportReady
                        } catch (e: Exception) {
                            debugIsError = true
                            debugStatus = try {
                                String.format(exportFailedTemplate, e.message ?: e.toString())
                            } catch (_: Exception) {
                                // Fallback if formatting fails
                                "${exportFailedTemplate} ${e.message ?: e.toString()}"
                            }
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
                    Text(stringResource(R.string.export_debug_zip))
                }
            }

            if (debugStatus.isNotEmpty()) {
                Text(
                    text = debugStatus,
                    color = if (debugIsError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(top = 8.dp),
                    style = MaterialTheme.typography.bodySmall
                )
            }
        }
    }
}
