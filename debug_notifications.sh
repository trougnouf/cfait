#!/bin/bash

# Debug notification issues on Android
# Usage: ./debug_notifications.sh

echo "========================================"
echo "Cfait Notification Debugger"
echo "========================================"
echo ""

# Check if device is connected
if ! adb devices | grep -q "device$"; then
    echo "❌ No Android device found. Please connect a device or start an emulator."
    exit 1
fi

echo "✓ Device connected"
echo ""

# Get device info
echo "📱 Device Information:"
echo "----------------------------------------"
ANDROID_VERSION=$(adb shell getprop ro.build.version.sdk)
TIMEZONE=$(adb shell getprop persist.sys.timezone)
CURRENT_TIME=$(adb shell date)
echo "Android API Level: $ANDROID_VERSION"
echo "Timezone: $TIMEZONE"
echo "Current Time: $CURRENT_TIME"
echo ""

# Check if app is installed
if ! adb shell pm list packages | grep -q "com.cfait"; then
    echo "❌ Cfait app not installed. Please install first."
    exit 1
fi

echo "✓ Cfait app installed"
echo ""

# Check permissions
echo "🔐 Checking Permissions:"
echo "----------------------------------------"
NOTIF_PERM=$(adb shell dumpsys package com.cfait | grep -A 1 "android.permission.POST_NOTIFICATIONS" | grep "granted=true")
if [ -n "$NOTIF_PERM" ]; then
    echo "✓ POST_NOTIFICATIONS: granted"
else
    echo "⚠️  POST_NOTIFICATIONS: not granted (might be API <33)"
fi

ALARM_PERM=$(adb shell dumpsys package com.cfait | grep -A 1 "android.permission.SCHEDULE_EXACT_ALARM" | grep "granted=true")
if [ -n "$ALARM_PERM" ]; then
    echo "✓ SCHEDULE_EXACT_ALARM: granted"
else
    echo "⚠️  SCHEDULE_EXACT_ALARM: not granted or not needed (API <31)"
fi
echo ""

# Check if alarms can be scheduled
if [ "$ANDROID_VERSION" -ge 31 ]; then
    echo "📅 Checking Alarm Scheduling Capability:"
    echo "----------------------------------------"
    CAN_SCHEDULE=$(adb shell dumpsys alarm | grep -A 50 "com.cfait" | grep -c "RTC_WAKEUP")
    if [ "$CAN_SCHEDULE" -gt 0 ]; then
        echo "✓ App can schedule exact alarms"
    else
        echo "⚠️  No active alarms found"
    fi
    echo ""
fi

# Check battery optimization
echo "🔋 Battery Optimization:"
echo "----------------------------------------"
if adb shell dumpsys deviceidle whitelist | grep -q "com.cfait"; then
    echo "✓ App is whitelisted (battery optimization disabled)"
else
    echo "⚠️  App is NOT whitelisted"
    echo "   To whitelist for testing:"
    echo "   adb shell dumpsys deviceidle whitelist +com.cfait"
fi
echo ""

# Clear logs and start monitoring
echo "📋 Starting log monitor..."
echo "----------------------------------------"
echo "Watching for: CfaitAlarm, CfaitMain, cfait_mobile"
echo ""
echo "Now add a task with a reminder in the app!"
echo "Example: test notif rem:$(date -d '+2 minutes' '+%H:%M' 2>/dev/null || date -v +2M '+%H:%M' 2>/dev/null || echo 'HH:MM')"
echo ""
echo "Press Ctrl+C to stop monitoring"
echo "========================================"
echo ""

# Clear old logs and start monitoring
adb logcat -c
adb logcat -v time | grep --line-buffered -E "CfaitAlarm|CfaitMain|cfait_mobile" | while read -r line; do
    # Color code the output
    if echo "$line" | grep -q "✓"; then
        echo -e "\033[0;32m$line\033[0m"  # Green for success
    elif echo "$line" | grep -q "✗"; then
        echo -e "\033[0;31m$line\033[0m"  # Red for errors
    elif echo "$line" | grep -q "⚠"; then
        echo -e "\033[0;33m$line\033[0m"  # Yellow for warnings
    else
        echo "$line"
    fi
done
