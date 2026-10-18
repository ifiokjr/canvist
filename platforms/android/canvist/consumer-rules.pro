# JNA and the UniFFI bindings are looked up reflectively from native code.
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keep class io.github.ifiokjr.canvist.** { *; }
-dontwarn java.awt.**
