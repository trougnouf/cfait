# Keep all classes in the uniffi-generated package
-keep class com.cfait.core.** { *; }
# Keep JNA classes needed for UniFFI
-keep class com.sun.jna.** { *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**