// Screen for selecting which calendar to import ICS file into.
package com.trougnouf.cfait.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.res.stringResource
import com.trougnouf.cfait.core.CfaitMobile
import com.trougnouf.cfait.core.MobileCalendar
import com.trougnouf.cfait.R
import kotlinx.coroutines.launch

@Composable
fun IcsImportScreen(
    api: CfaitMobile,
    icsContent: String,
    calendars: List<MobileCalendar>,
    onImportComplete: (String) -> Unit,
    onCancel: () -> Unit
) {
    var selectedCalendar by remember { mutableStateOf<String?>(null) }
    var taskCount by remember { mutableStateOf<Int?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    // Parse ICS content to count tasks
    LaunchedEffect(icsContent) {
        try {
            // Simple count of VTODO entries
            taskCount = icsContent.split("BEGIN:VTODO").size - 1
        } catch (e: Exception) {
            errorMessage = "Failed to parse ICS file: ${e.message}"
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
            .padding(bottom = 48.dp), // Extra padding to avoid navigation bar
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        // Header
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                stringResource(R.string.import_action),
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold
            )
        }

        // Info card
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.primaryContainer
            )
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    NfIcon(NfIcons.IMPORT, 20.sp, MaterialTheme.colorScheme.onPrimaryContainer)
                    Text(
                        stringResource(R.string.ics_file_detected),
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }
                val currentTaskCount = taskCount
                if (currentTaskCount != null) {
                    Text(
                        stringResource(R.string.found_tasks_to_import, currentTaskCount),
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }
                if (errorMessage != null) {
                    Text(
                        errorMessage!!,
                        color = MaterialTheme.colorScheme.error
                    )
                }
            }
        }

        // Calendar selection
        Text(
            stringResource(R.string.select_target_collection),
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold
        )

        // Calendar list
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        ) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                items(calendars.filter { !it.isDisabled }) { calendar ->
                    CalendarSelectionItem(
                        calendar = calendar,
                        isSelected = selectedCalendar == calendar.href,
                        onClick = { selectedCalendar = calendar.href }
                    )
                }
            }
        }

        // Action buttons
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedButton(
                onClick = onCancel,
                modifier = Modifier.weight(1f)
            ) {
                Text(androidx.compose.ui.res.stringResource(R.string.cancel))
            }
            Button(
                onClick = {
                    selectedCalendar?.let { href ->
                        onImportComplete(href)
                    }
                },
                modifier = Modifier.weight(1f),
                enabled = selectedCalendar != null && (taskCount ?: 0) > 0
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    NfIcon(NfIcons.IMPORT, 16.sp, MaterialTheme.colorScheme.onPrimary)
                    Text(androidx.compose.ui.res.stringResource(R.string.import_action))
                }
            }
        }
    }
}

@Composable
fun CalendarSelectionItem(
    calendar: MobileCalendar,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(
            containerColor = if (isSelected) {
                MaterialTheme.colorScheme.secondaryContainer
            } else {
                MaterialTheme.colorScheme.surface
            }
        ),
        border = if (isSelected) {
            androidx.compose.foundation.BorderStroke(
                2.dp,
                MaterialTheme.colorScheme.secondary
            )
        } else null
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            RadioButton(
                selected = isSelected,
                onClick = onClick
            )
            Column(
                modifier = Modifier.weight(1f)
            ) {
                Text(
                    text = calendar.name,
                    fontWeight = FontWeight.Medium,
                    fontSize = 16.sp
                )
                Text(
                    text = if (calendar.isLocal) stringResource(R.string.local_collection) else calendar.href,
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}
