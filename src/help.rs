// File: ./src/help.rs
#[cfg_attr(feature = "mobile", derive(uniffi::Enum))]
#[derive(PartialEq, Clone, Copy, Debug, Default)]
pub enum HelpTab {
    #[default]
    Syntax,
    Keyboard,
}

#[derive(Clone, Debug)]
pub struct HelpItem {
    pub keys: &'static str,
    pub desc: &'static str,
    pub example: &'static str,
}

#[derive(Clone, Debug)]
pub struct HelpSection {
    pub title: &'static str,
    pub items: &'static [HelpItem],
}

pub const SYNTAX_HELP: &[HelpSection] = &[
    HelpSection {
        title: "Organization",
        items: &[
            HelpItem {
                keys: "!1..9",
                desc: "Priority high (1) to low (9)",
                example: "!1, !5, !9",
            },
            HelpItem {
                keys: "#tag",
                desc: "Add category. Use ':' for sub-tags",
                example: "#work, #dev:backend",
            },
            HelpItem {
                keys: "@@loc",
                desc: "Location. Supports hierarchy",
                example: "@@home, @@store:aldi",
            },
            HelpItem {
                keys: "~duration",
                desc: "Estimated duration (Single or Range)",
                example: "~30m, ~1.5h, ~15m-45m",
            },
            HelpItem {
                keys: "spent:X",
                desc: "Track time spent",
                example: "spent:1h, spent:30m",
            },
            HelpItem {
                keys: "done:date",
                desc: "Set completion date explicitly",
                example: "done:2024-01-01 15:30",
            },
            HelpItem {
                keys: "#a:=#b",
                desc: "Define/update tag alias inline",
                example: "#tree:=#gardening,@@home",
            },
            HelpItem {
                keys: "@@a:=#b",
                desc: "Define/update location alias",
                example: "@@aldi:=#groceries,#shopping",
            },
            HelpItem {
                keys: "\\#text",
                desc: "Escape special characters",
                example: "\\#not-a-tag",
            },
        ],
    },
    HelpSection {
        title: "Timeline",
        items: &[
            HelpItem {
                keys: "@date",
                desc: "Due date. Deadline",
                example: "@tomorrow, @2025-12-31",
            },
            HelpItem {
                keys: "^date",
                desc: "Start date. Hides until date",
                example: "^next week, ^2025-01-01",
            },
            HelpItem {
                keys: "Offsets",
                desc: "Add time from today",
                example: "1d, 2w, 3mo",
            },
            HelpItem {
                keys: "Weekdays",
                desc: "Next occurrence ('next' optional)",
                example: "@friday, @next monday",
            },
            HelpItem {
                keys: "Next period",
                desc: "Next week/month/year",
                example: "@next week, @next month",
            },
            HelpItem {
                keys: "Keywords",
                desc: "Relative dates supported",
                example: "today, tomorrow",
            },
            HelpItem {
                keys: "^@date",
                desc: "Set both Start and Due dates",
                example: "^@tomorrow, ^@2d",
            },
        ],
    },
    HelpSection {
        title: "Recurrence",
        items: &[
            HelpItem {
                keys: "@daily",
                desc: "Quick presets",
                example: "@daily, @weekly, @monthly, @yearly",
            },
            HelpItem {
                keys: "@every X",
                desc: "Custom intervals",
                example: "@every 3 days, @every 2 weeks",
            },
            HelpItem {
                keys: "@every <day>",
                desc: "Specific weekdays",
                example: "@every monday,wednesday",
            },
            HelpItem {
                keys: "until <date>",
                desc: "End date for recurrence",
                example: "@daily until 2025-12-31",
            },
            HelpItem {
                keys: "except <date>",
                desc: "Skip specific dates",
                example: "@daily except 2025-12-25",
            },
            HelpItem {
                keys: "except day",
                desc: "Exclude weekdays",
                example: "except mo,tue",
            },
            HelpItem {
                keys: "except month",
                desc: "Exclude months",
                example: "except oct,nov",
            },
        ],
    },
    HelpSection {
        title: "Metadata",
        items: &[
            HelpItem {
                keys: "url:",
                desc: "Attach a link",
                example: "url:https://perdu.com",
            },
            HelpItem {
                keys: "geo:",
                desc: "Coordinates (lat,long)",
                example: "geo:53.04,-121.10",
            },
            HelpItem {
                keys: "desc:",
                desc: "Append description text",
                example: "desc:\"Call back later\"",
            },
            HelpItem {
                keys: "rem:10m",
                desc: "Relative reminder (before due/start)",
                example: "Adjusts if date changes",
            },
            HelpItem {
                keys: "rem:in 5m",
                desc: "Relative from now (becomes absolute)",
                example: "rem:in 2h",
            },
            HelpItem {
                keys: "rem:date",
                desc: "Absolute reminder (fixed time)",
                example: "rem:2025-01-20 9am",
            },
            HelpItem {
                keys: "+cal",
                desc: "Force calendar event creation",
                example: "Task @tomorrow +cal",
            },
            HelpItem {
                keys: "-cal",
                desc: "Prevent calendar event creation",
                example: "Task @tomorrow -cal",
            },
        ],
    },
    HelpSection {
        title: "Search & Filtering",
        items: &[
            HelpItem {
                keys: "text",
                desc: "Matches summary or description",
                example: "buy cat food",
            },
            HelpItem {
                keys: "#tag",
                desc: "Filter by specific tag",
                example: "#gardening",
            },
            HelpItem {
                keys: "@@loc",
                desc: "Filter by specific location",
                example: "@@home",
            },
            HelpItem {
                keys: "is:ready",
                desc: "Work Mode - actionable tasks only",
                example: "Not done, started, not blocked",
            },
            HelpItem {
                keys: "is:status",
                desc: "Filter by state",
                example: "is:done, is:started, is:active, is:blocked",
            },
            HelpItem {
                keys: "< > <=",
                desc: "Compare operators for filters",
                example: "~<20m, !<4",
            },
            HelpItem {
                keys: "Dates",
                desc: "Filter by timeframe",
                example: "@<today (Overdue), ^>1w",
            },
            HelpItem {
                keys: "Date!",
                desc: "Include unset dates with '!' suffix",
                example: "@<today!",
            },
            HelpItem {
                keys: "(A | B) -C",
                desc: "Boolean logic (AND, OR, NOT)",
                example: "(#work | #school) -is:done",
            },
        ],
    },
];

pub const KEYBOARD_HELP: &[HelpSection] = &[
    HelpSection {
        title: "Global & Navigation",
        items: &[
            HelpItem {
                keys: "?",
                desc: "Toggle help",
                example: "",
            },
            HelpItem {
                keys: "q",
                desc: "Quit",
                example: "",
            },
            HelpItem {
                keys: "Tab",
                desc: "Switch focus (Sidebar/Main)",
                example: "",
            },
            HelpItem {
                keys: "j / k",
                desc: "Move selection down / up",
                example: "",
            },
            HelpItem {
                keys: "PgDn / PgUp",
                desc: "Scroll page down / up",
                example: "",
            },
        ],
    },
    HelpSection {
        title: "Task Actions",
        items: &[
            HelpItem {
                keys: "a",
                desc: "Add new task",
                example: "",
            },
            HelpItem {
                keys: "e",
                desc: "Edit task title (Smart syntax)",
                example: "",
            },
            HelpItem {
                keys: "E",
                desc: "Edit task description",
                example: "",
            },
            HelpItem {
                keys: "Space",
                desc: "Toggle done state",
                example: "",
            },
            HelpItem {
                keys: "d",
                desc: "Delete selected task",
                example: "",
            },
            HelpItem {
                keys: "s",
                desc: "Start / Pause active tracking",
                example: "",
            },
            HelpItem {
                keys: "S",
                desc: "Stop / Reset active tracking",
                example: "",
            },
            HelpItem {
                keys: "x",
                desc: "Cancel task",
                example: "",
            },
            HelpItem {
                keys: "+ / -",
                desc: "Increase / Decrease priority",
                example: "",
            },
            HelpItem {
                keys: "M",
                desc: "Move to different calendar",
                example: "",
            },
            HelpItem {
                keys: "y",
                desc: "Yank task (Link/Move)",
                example: "",
            },
        ],
    },
    HelpSection {
        title: "Hierarchy & Relations",
        items: &[
            HelpItem {
                keys: "c",
                desc: "Link yanked as child",
                example: "",
            },
            HelpItem {
                keys: "C",
                desc: "Create new subtask",
                example: "",
            },
            HelpItem {
                keys: "b",
                desc: "Mark selected as blocked by yanked",
                example: "",
            },
            HelpItem {
                keys: "l",
                desc: "Relate selected to yanked",
                example: "",
            },
            HelpItem {
                keys: "> / .",
                desc: "Demote (make child of task above)",
                example: "",
            },
            HelpItem {
                keys: "< / ,",
                desc: "Promote (remove parent)",
                example: "",
            },
            HelpItem {
                keys: "L",
                desc: "Jump to related tasks menu",
                example: "",
            },
            HelpItem {
                keys: "Enter",
                desc: "Toggle completed subtasks expansion",
                example: "",
            },
        ],
    },
    HelpSection {
        title: "Sidebar & Filters",
        items: &[
            HelpItem {
                keys: "/",
                desc: "Focus search bar",
                example: "",
            },
            HelpItem {
                keys: "1, 2, 3",
                desc: "Switch sidebar tab (Calendars, Tags, Locations)",
                example: "",
            },
            HelpItem {
                keys: "m",
                desc: "Toggle tag match logic (AND / OR)",
                example: "",
            },
            HelpItem {
                keys: "H",
                desc: "Toggle hide completed tasks",
                example: "",
            },
            HelpItem {
                keys: "Space",
                desc: "Toggle calendar visibility (in Calendars tab)",
                example: "",
            },
            HelpItem {
                keys: "Enter",
                desc: "Toggle tag/location selection",
                example: "",
            },
            HelpItem {
                keys: "*",
                desc: "Clear all filters / Show all calendars",
                example: "",
            },
            HelpItem {
                keys: "Right",
                desc: "Isolate calendar / Focus tag or location",
                example: "",
            },
        ],
    },
    HelpSection {
        title: "Misc",
        items: &[
            HelpItem {
                keys: "r",
                desc: "Force sync refresh",
                example: "",
            },
            HelpItem {
                keys: "R",
                desc: "Jump to random actionable task",
                example: "",
            },
            HelpItem {
                keys: "X",
                desc: "Export local tasks to remote calendar",
                example: "",
            },
            HelpItem {
                keys: "Esc",
                desc: "Clear search, yank, or close menus",
                example: "",
            },
        ],
    },
];
