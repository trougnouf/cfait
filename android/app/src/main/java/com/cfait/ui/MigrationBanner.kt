// Compose UI component for package migration banner.
package com.cfait.ui

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate

private const val PREFS_NAME = "cfait_prefs"
private const val KEY_LAST_SHOWN_DATE = "migration_banner_last_shown_date"

@Composable
fun MigrationBanner() {
    val context = LocalContext.current
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    var isDismissed by remember { mutableStateOf(false) }

    // Check if we should show the banner (after 2026-01-13)
    val currentDate = LocalDate.now()
    val migrationDate = LocalDate.of(2026, 1, 13)

    // Check if already shown today
    val lastShownDate = prefs.getString(KEY_LAST_SHOWN_DATE, null)
    val alreadyShownToday = lastShownDate == currentDate.toString()

    if (currentDate.isBefore(migrationDate) || isDismissed || alreadyShownToday) {
        return
    }

    // Mark as shown today when banner is displayed
    LaunchedEffect(Unit) {
        prefs.edit().putString(KEY_LAST_SHOWN_DATE, currentDate.toString()).apply()
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(8.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer
        )
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.weight(1f)
                ) {
                    NfIcon(NfIcons.INFO, 20.sp, MaterialTheme.colorScheme.onErrorContainer)
                    Text(
                        "Important: App Package Change",
                        fontWeight = FontWeight.Bold,
                        fontSize = 16.sp,
                        color = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
                IconButton(
                    onClick = { isDismissed = true },
                    modifier = Modifier.size(32.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.Close,
                        contentDescription = "Dismiss banner",
                        tint = MaterialTheme.colorScheme.onErrorContainer
                    )
                }
            }

            Text(
                "This app is being renamed from com.cfait to com.trougnouf.cfait. You will need to install the new version separately.",
                fontSize = 14.sp,
                color = MaterialTheme.colorScheme.onErrorContainer
            )

            HorizontalDivider(
                modifier = Modifier.padding(vertical = 4.dp),
                color = MaterialTheme.colorScheme.onErrorContainer.copy(alpha = 0.3f)
            )

            Text(
                "If you use CalDAV sync:",
                fontWeight = FontWeight.SemiBold,
                fontSize = 14.sp,
                color = MaterialTheme.colorScheme.onErrorContainer
            )
            Text(
                "• Simply install the new app from F-Droid or Play Store\n" +
                        "• Your tasks will sync automatically\n" +
                        "• You can then uninstall this old version",
                fontSize = 13.sp,
                color = MaterialTheme.colorScheme.onErrorContainer
            )

            Text(
                "If you use local calendars:",
                fontWeight = FontWeight.SemiBold,
                fontSize = 14.sp,
                color = MaterialTheme.colorScheme.onErrorContainer,
                modifier = Modifier.padding(top = 4.dp)
            )

            Text(
                buildAnnotatedString {
                    append("• Go to Settings → Local Calendars\n")
                    append("• Export ")
                    withStyle(style = SpanStyle(fontFamily = NerdFont)) {
                        append(NfIcons.EXPORT)
                    }
                    append(" each calendar\n")
                    append("• Install the new app from F-Droid or Play Store\n")
                    append("• Import ")
                    withStyle(style = SpanStyle(fontFamily = NerdFont)) {
                        append(NfIcons.IMPORT)
                    }
                    append(" your calendars in the new app\n")
                    append("• Then uninstall this old version")
                },
                fontSize = 13.sp,
                color = MaterialTheme.colorScheme.onErrorContainer
            )

            Text(
                "Note: Configuration (settings) will need to be re-entered in the new app.",
                fontSize = 12.sp,
                fontStyle = androidx.compose.ui.text.font.FontStyle.Italic,
                color = MaterialTheme.colorScheme.onErrorContainer,
                modifier = Modifier.padding(top = 4.dp)
            )
        }
    }
}
