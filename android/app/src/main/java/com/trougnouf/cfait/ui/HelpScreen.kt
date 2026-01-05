// Compose UI screen for help documentation.
package com.trougnouf.cfait.ui

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.trougnouf.cfait.BuildConfig
import com.trougnouf.cfait.R
import kotlinx.coroutines.launch
import kotlin.random.Random

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HelpScreen(onBack: () -> Unit) {
    val clipboard = LocalClipboard.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    fun copy(text: String) {
        scope.launch {
            clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("Donation Address", text)))
            Toast.makeText(context, "Copied to clipboard", Toast.LENGTH_SHORT).show()
        }
    }

    fun openUrl(url: String) {
        try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            context.startActivity(intent)
        } catch (e: Exception) {
            Toast.makeText(context, "Could not open URL", Toast.LENGTH_SHORT).show()
        }
    }

    // Randomize icon (choose between 3 icons)
    val helpIconRes = when (Random.nextInt(3)) {
        0 -> R.drawable.nf_cod_question_breeze_face_hugs
        1 -> R.drawable.nf_md_robot_confused_breeze_face_hugs
        else -> R.drawable.nf_md_robot_confused_help_breeze_face_hugs
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Image(
                            painter = painterResource(id = helpIconRes),
                            contentDescription = null,
                            modifier = Modifier.width(52.5.dp).height(20.dp)
                        )
                        Spacer(Modifier.width(8.dp))
                        Text("Help & About")
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) { NfIcon(NfIcons.BACK, 20.sp) }
                },
            )
        },
    ) { p ->
        LazyColumn(
            modifier = Modifier.padding(p).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item {
                HelpSection(
                    "Organization",
                    NfIcons.TAG,
                    listOf(
                        HelpItem("!1", "Priority high (1) to low (9)", "!1, !5, !9"),
                        HelpItem(
                            "#tag",
                            "Add category. Use ':' for sub-tags.",
                            "#work, #dev:backend, #work:project:urgent"
                        ),
                        HelpItem(
                            "#a:=#b,#c,@@d",
                            "Define/update tag alias inline (retroactive).",
                            "#tree_planting:=#gardening,@@home"
                        ),
                        HelpItem(
                            "@@a:=#b,#c",
                            "Define/update location alias (retroactive).",
                            "@@aldi:=#groceries,#shopping"
                        ),
                        HelpItem("~30m", "Estimated duration (m/h/d/w).", "~30m, ~1.5h, ~2d"),
                        HelpItem(
                            "@@loc",
                            "Location. Supports hierarchy with ':'.",
                            "@@home, @@home:office, @@store:aldi:downtown"
                        ),
                        HelpItem("\\#text", "Escape special characters.", "\\#not-a-tag \\@not-a-date"),
                    ),
                )
            }

            item {
                HelpSection(
                    "Timeline",
                    NfIcons.CALENDAR,
                    listOf(
                        HelpItem("@date", "Due date. Deadline.", "@tomorrow, @2025-12-31"),
                        HelpItem("^date", "Start date. Hides until date.", "^next week, ^2025-01-01"),
                        HelpItem("Offsets", "Add time from today.", "1d, 2w, 3mo (optional: @2 weeks = @in 2 weeks)"),
                        HelpItem("Weekdays", "Next occurrence (\"next\" optional).", "@friday = @next friday, @monday"),
                        HelpItem("Next period", "Next week/month/year.", "@next week, @next month, @next year"),
                        HelpItem("Keywords", "Relative dates supported.", "today, tomorrow"),
                    ),
                )
            }

            item {
                HelpSection(
                    "Recurrence",
                    NfIcons.REPEAT,
                    listOf(
                        HelpItem("@daily", "Quick presets.", "@daily, @weekly, @monthly"),
                        HelpItem("@every X", "Custom intervals.", "@every 3 days, @every 2 weeks"),
                        HelpItem("@every <day>", "Specific weekdays.", "@every monday, @every monday,wednesday,friday"),
                        HelpItem("until", "End date for recurrence.", "@daily until 2025-12-31"),
                        HelpItem("except", "Skip specific dates.", "except 2025-12-25,2026-01-01"),
                        HelpItem("except day", "Exclude weekdays.", "except mo,tue or except saturdays,sundays"),
                        HelpItem("except month", "Exclude months.", "except oct,nov,dec or except march"),
                    ),
                )
            }

            item {
                HelpSection(
                    "Metadata",
                    NfIcons.INFO,
                    listOf(
                        HelpItem("url:", "Attach a link.", "url:https://perdu.com"),
                        HelpItem("geo:", "Coordinates (lat,long).", "geo:53.046070, -121.105264"),
                        HelpItem("desc:", "Append description text.", "desc:\"Call back later\""),
                        HelpItem("rem:10m", "Relative reminder (before due date).", "Adjusts if due date changes"),
                        HelpItem(
                            "rem:in 5m",
                            "Relative from now (becomes absolute).",
                            "rem:in 2h (5 min/2 hours from now)"
                        ),
                        HelpItem(
                            "rem:next friday",
                            "Next occurrence (becomes absolute).",
                            "rem:next week, rem:next month"
                        ),
                        HelpItem(
                            "rem:8am",
                            "Absolute reminder (fixed time).",
                            "rem:2025-01-20 9am, rem:2025-12-31 10:00"
                        ),
                        HelpItem("+cal", "Force calendar event creation.", "Task @tomorrow +cal"),
                        HelpItem("-cal", "Prevent calendar event creation.", "Private task @tomorrow -cal"),
                    ),
                )
            }

            item {
                HelpSection(
                    "Search & Filtering",
                    NfIcons.SEARCH,
                    listOf(
                        HelpItem("text", "Matches summary or description.", "buy cat food"),
                        HelpItem("#tag", "Filter by specific tag.", "#gardening"),
                        HelpItem(
                            "is:ready",
                            "Work Mode - actionable tasks only.",
                            "Not done, start date passed, not blocked"
                        ),
                        HelpItem("is:status", "Filter by state.", "is:done, is:started, is:active"),
                        HelpItem(
                            "Operators",
                            "Compare values (<, >, <=, >=).",
                            "~<20m (less than 20 min), !<4 (urgent)"
                        ),
                        HelpItem("  Dates", "Filter by timeframe.", "@<today (Overdue), ^>1w (Start 1+ weeks)"),
                        HelpItem(
                            "  Date!",
                            "Include unset dates with '!' suffix.",
                            "@<today! (Overdue OR no due date)"
                        ),
                        HelpItem("  Priority", "Filter by priority range.", "!<3 (High prio), !>=5"),
                        HelpItem("  Duration", "Filter by effort.", "~<15m (Quick tasks)"),
                        HelpItem("  Location", "Filter by location (matches sub-locations).", "@@home, @@store:aldi"),
                        HelpItem(
                            "Combine",
                            "Mix multiple filters.",
                            "is:ready #work ~<1h (Actionable work tasks under 1 hour)"
                        ),
                    ),
                )
            }

            item {
                Card(
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(
                            alpha = 0.5f
                        )
                    )
                ) {
                    Column(Modifier.padding(12.dp)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.padding(bottom = 8.dp)
                        ) {
                            NfIcon(NfIcons.HEART_HAND, 18.sp, Color(0xFFE57373))
                            Spacer(Modifier.width(8.dp))
                            Text(
                                "Support Development",
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }
                        HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
                        Spacer(Modifier.height(8.dp))

                        DonationRow(
                            icon = NfIcons.CREDIT_CARD,
                            name = "Liberapay",
                            value = "https://liberapay.com/trougnouf",
                            trailingIcon = { NfIcon(NfIcons.EXTERNAL_LINK, 14.sp, Color.Gray) },
                        ) { openUrl("https://liberapay.com/trougnouf") }
                        DonationRow(NfIcons.BANK, "Bank (SEPA)", "BE77 9731 6116 6342") { copy("BE77 9731 6116 6342") }
                        DonationRow(
                            NfIcons.BITCOIN,
                            "Bitcoin",
                            "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979",
                        ) { copy("bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979") }
                        DonationRow(NfIcons.LITECOIN, "Litecoin", "ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg") {
                            copy("ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg")
                        }
                        DonationRow(NfIcons.ETHEREUM, "Ethereum", "0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB") {
                            copy("0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB")
                        }
                    }
                }
            }

            item {
                Spacer(Modifier.height(16.dp))
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        "Cfait v${BuildConfig.VERSION_NAME} • GPL3",
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.Gray
                    )
                    Text("Trougnouf (Benoit Brummer)", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
                    Text(
                        text = "https://codeberg.org/trougnouf/cfait",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.clickable { openUrl("https://codeberg.org/trougnouf/cfait") },
                    )
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
fun HelpSection(
    title: String,
    icon: String,
    items: List<HelpItem>,
) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)),
    ) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                NfIcon(icon, 18.sp, MaterialTheme.colorScheme.primary)
                Spacer(Modifier.width(8.dp))
                Text(title, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary)
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.2f))
            Spacer(Modifier.height(8.dp))
            items.forEach { item ->
                HelpRow(item)
                if (item != items.last()) Spacer(Modifier.height(8.dp))
            }
        }
    }
}

@Composable
fun HelpRow(item: HelpItem) {
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier =
                    Modifier
                        .background(MaterialTheme.colorScheme.tertiaryContainer, RoundedCornerShape(4.dp))
                        .padding(horizontal = 6.dp, vertical = 2.dp),
            ) {
                Text(
                    item.syntax,
                    style = MaterialTheme.typography.labelMedium,
                    fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                    color = MaterialTheme.colorScheme.onTertiaryContainer,
                )
            }
            Spacer(Modifier.width(8.dp))
            Text(item.desc, style = MaterialTheme.typography.bodySmall, modifier = Modifier.weight(1f))
        }
        if (item.example.isNotEmpty()) {
            Text(
                "e.g. ${item.example}",
                style = MaterialTheme.typography.bodySmall,
                color = Color.Gray,
                fontSize = 10.sp,
                modifier = Modifier.padding(start = 0.dp, top = 2.dp),
            )
        }
    }
}

@Composable
fun DonationRow(
    icon: String,
    name: String,
    value: String,
    trailingIcon: @Composable () -> Unit = { NfIcon(NfIcons.COPY, 14.sp, Color.Gray) },
    onClick: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable { onClick() }
                .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        NfIcon(icon, 16.sp, Color.Gray)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurface)
            Text(value, style = MaterialTheme.typography.bodySmall, color = Color.Gray, fontSize = 10.sp, maxLines = 1)
        }
        trailingIcon()
    }
}

data class HelpItem(
    val syntax: String,
    val desc: String,
    val example: String = "",
)
