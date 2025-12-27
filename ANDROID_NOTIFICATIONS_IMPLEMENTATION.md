# Android Notifications Implementation

## Overview

This document summarizes the implementation of native Android notifications for Cfait, enabling task reminders to survive app closure and device restarts.

## Architecture

The implementation consists of three main components:

1. **Rust Backend (mobile.rs)**: Exposes blocking methods for calculating next alarm times and retrieving currently firing alarms
2. **Android Manifest**: Declares permissions and registers broadcast receivers
3. **Kotlin Components**: Manages AlarmManager scheduling and notification display

## Files Modified/Created

### Rust Layer

#### `src/mobile.rs`
- **Added `MobileAlarmInfo` struct**: Contains task_uid, alarm_uid, title, and body for notification display
- **Added `get_next_alarm_timestamp()`**: Synchronous method that calculates the next alarm time (explicit or implicit) across all tasks. Returns Unix timestamp in seconds.
- **Added `get_firing_alarms()`**: Synchronous method that returns all alarms currently due (within 1-hour grace period), including both explicit and implicit alarms.

Key features:
- Uses `blocking_lock()` for synchronous access to task store
- Handles both explicit alarms (from VALARM) and implicit alarms (auto-reminders for due/start dates)
- Respects `auto_reminders` config flag
- Filters out completed tasks and acknowledged alarms
- Generates synthetic alarm IDs for implicit alarms matching the format in `system.rs`

### Android Layer

#### `android/app/src/main/AndroidManifest.xml`
Added permissions:
- `POST_NOTIFICATIONS`: Show notifications to user
- `SCHEDULE_EXACT_ALARM` & `USE_EXACT_ALARM`: Schedule precise alarm times
- `RECEIVE_BOOT_COMPLETED`: Reschedule alarms after reboot
- `WAKE_LOCK`: Wake device when alarm fires

Registered receivers:
- `AlarmReceiver`: Handles system alarm wakeups
- `NotificationActionReceiver`: Handles snooze/dismiss button clicks
- `BootReceiver`: Reschedules alarms after device restart

#### `android/app/src/main/java/com/cfait/util/AlarmScheduler.kt`
Utility object for scheduling Android system alarms:
- Calls Rust `get_next_alarm_timestamp()` to determine when to wake
- Uses `AlarmManager.setExactAndAllowWhileIdle()` for battery-efficient exact timing
- Handles Android 12+ permission checks for exact alarms
- Replaces any existing alarm with `FLAG_UPDATE_CURRENT`

#### `android/app/src/main/java/com/cfait/receivers/AlarmReceiver.kt`
Handles system wakeups:
1. Initializes `CfaitMobile` API (app may be dead)
2. Calls `get_firing_alarms()` to retrieve due alarms
3. Creates notification channel if needed
4. Displays notification for each firing alarm with:
   - Task title and alarm description
   - Tap action to open app
   - "Snooze (15m)" action button
   - "Dismiss" action button
5. Schedules next alarm to continue the loop

#### `android/app/src/main/java/com/cfait/receivers/NotificationActionReceiver.kt`
Handles notification actions:
- **Snooze**: Calls `api.snoozeAlarm()` with 15-minute duration
- **Dismiss**: Calls `api.dismissAlarm()` to acknowledge the alarm
- Closes the notification
- Reschedules next alarm after handling

#### `android/app/src/main/java/com/cfait/receivers/BootReceiver.kt`
Responds to `ACTION_BOOT_COMPLETED`:
- Initializes API
- Calls `AlarmScheduler.scheduleNextAlarm()` to restore alarm schedule

#### `android/app/src/main/java/com/cfait/MainActivity.kt`
Modified `fastStart()` function:
- After successful sync, calls `AlarmScheduler.scheduleNextAlarm()`
- Ensures alarm schedule stays current when app is open

## How It Works

### Scheduling Flow
1. User syncs or app starts → `fastStart()` runs
2. `AlarmScheduler.scheduleNextAlarm()` is called
3. Rust calculates next alarm time (explicit + implicit)
4. Android `AlarmManager` is scheduled to wake at that time
5. System sleeps until alarm time

### Firing Flow
1. System time reaches scheduled alarm
2. `AlarmReceiver.onReceive()` is called
3. Rust returns list of currently firing alarms
4. Notifications are displayed with action buttons
5. Next alarm is scheduled (loop continues)

### Action Flow
1. User taps "Snooze" or "Dismiss"
2. `NotificationActionReceiver` handles the action
3. Rust updates task state (snooze/dismiss)
4. Notification is cancelled
5. Next alarm is recalculated and scheduled

### Boot Flow
1. Device restarts
2. `BootReceiver` receives `BOOT_COMPLETED` broadcast
3. Alarm schedule is restored from current task state

## Implicit vs Explicit Alarms

- **Explicit alarms**: Defined in task VALARM components
- **Implicit alarms**: Auto-generated for tasks with due/start dates when `auto_reminders` is enabled

Both types are handled identically in the Android layer. The Rust layer distinguishes them by:
- Explicit alarms have real UIDs from the alarm object
- Implicit alarms have synthetic IDs: `implicit_{type}:|{timestamp}|{task_uid}`

## Configuration

Uses existing config values:
- `auto_reminders`: Enable/disable implicit alarms
- `default_reminder_time`: Time for all-day events (format: "HH:MM")
- Snooze duration is hardcoded to 15 minutes in notification actions

## Notes

- Notifications use `PRIORITY_HIGH` for heads-up display
- Grace period of 60 minutes for firing alarms prevents spam from old alarms
- Unique notification IDs prevent duplicate notifications
- Works even when app is closed or device is sleeping
- Survives device restarts
- Battery optimized with `setExactAndAllowWhileIdle()`

## Testing

To test the implementation:
1. Create a task with a near-future due date
2. Enable auto-reminders in settings
3. Close the app completely
4. Wait for the due time
5. Device should wake and show notification
6. Test snooze/dismiss actions
7. Verify next alarm is scheduled

## Future Improvements

Potential enhancements:
- Configurable snooze durations in notification actions
- Custom notification sounds per calendar
- Notification channels per calendar for user control
- Rich notification content with task details
- Quick action to complete task from notification