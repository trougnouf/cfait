// SPDX-License-Identifier: GPL-3.0-or-later
package com.trougnouf.cfait.ui

import android.content.ClipData
import android.widget.Toast
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.then
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.R
import com.trougnouf.cfait.core.CfaitMobile
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TreeEditorScreen(
    api: CfaitMobile,
    uid: String,
    onBack: () -> Unit,
    onSaveComplete: () -> Unit
) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboard = LocalClipboard.current

    val markdownText = remember { TextFieldState() }
    var undoStack by remember { mutableStateOf(listOf<FieldSnapshot>()) }
    var redoStack by remember { mutableStateOf(listOf<FieldSnapshot>()) }

    var isLoading by remember { mutableStateOf(true) }
    var isSaving by remember { mutableStateOf(false) }
    val isDark = MaterialTheme.colorScheme.background.luminance() < 0.5f

    LaunchedEffect(uid) {
        try {
            val initVal = FieldSnapshot(
                withContext(Dispatchers.IO) { api.getTaskTreeMarkdown(uid) },
                TextRange(0)
            )
            markdownText.restore(initVal)
            undoStack = listOf(initVal)
            redoStack = emptyList()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            markdownText.restore(FieldSnapshot(context.getString(R.string.error_general, e.message ?: ""), TextRange(0)))
        } finally {
            isLoading = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.edit_tree_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.CROSS, 20.sp) }
                },
                actions = {
                    if (undoStack.size > 1) {
                        IconButton(onClick = {
                            if (undoStack.size > 1) {
                                val current = undoStack.last()
                                redoStack = (redoStack + current).takeLast(50)
                                undoStack = undoStack.dropLast(1)
                                markdownText.restore(undoStack.last())
                            }
                        }) { NfIcon(NfIcons.UNDO, 20.sp) }
                    }

                    if (redoStack.isNotEmpty()) {
                        IconButton(onClick = {
                            if (redoStack.isNotEmpty()) {
                                val next = redoStack.last()
                                redoStack = redoStack.dropLast(1)
                                undoStack = (undoStack + next).takeLast(50)
                                markdownText.restore(next)
                            }
                        }) { NfIcon(NfIcons.REDO, 20.sp) }
                    }

                    IconButton(
                        onClick = {
                            scope.launch {
                                clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("tree_markdown", markdownText.text.toString())))
                                Toast.makeText(context, context.getString(R.string.copied_to_clipboard), Toast.LENGTH_SHORT).show()
                            }
                        },
                        enabled = !isLoading
                    ) {
                        NfIcon(NfIcons.COPY, 20.sp)
                    }
                    IconButton(
                        onClick = {
                            isSaving = true
                            scope.launch {
                                try {
                                    withContext(Dispatchers.IO) {
                                        api.syncTaskTreeFromMarkdown(uid, markdownText.text.toString())
                                    }
                                    triggerBackgroundSync(context, api)
                                    onSaveComplete()
                                } catch (e: Exception) {
                                    Toast.makeText(context, context.getString(R.string.error_general, e.message ?: ""), Toast.LENGTH_LONG).show()
                                    isSaving = false
                                }
                            }
                        },
                        enabled = !isLoading && !isSaving
                    ) {
                        if (isSaving) {
                            CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                        } else {
                            NfIcon(NfIcons.CHECK, 20.sp, MaterialTheme.colorScheme.primary)
                        }
                    }
                }
            )
        }
    ) { padding ->
        if (isLoading) {
            Box(modifier = Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        } else {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .imePadding()
                    .padding(16.dp)
            ) {
                OutlinedTextField(
                    state = markdownText,
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    textStyle = TextStyle(fontSize = 14.sp),
                    inputTransformation = InputTransformation.listAutoIndent(api)
                        .then(markdownText.undoPushTransform { snap ->
                            undoStack = (undoStack + snap).takeLast(50)
                            redoStack = emptyList()
                        }),
                    outputTransformation = remember(isDark) {
                        MarkdownTransformation(isDark, api).asOutputTransformation()
                    },
                    keyboardOptions = KeyboardOptions.Default.copy(
                        keyboardType = KeyboardType.Text,
                        imeAction = ImeAction.None
                    )
                )
                CursorContextBanner(api, markdownText, uid, onNavigate = null)
            }
        }
    }
}
