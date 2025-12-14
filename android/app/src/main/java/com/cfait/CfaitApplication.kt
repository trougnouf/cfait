// File: ./android/app/src/main/java/com/cfait/CfaitApplication.kt
package com.cfait

import android.app.Application
import com.cfait.core.CfaitMobile

class CfaitApplication : Application() {
    lateinit var api: CfaitMobile
        private set

    override fun onCreate() {
        super.onCreate()
        // Initialize the Rust backend once for the lifetime of the application
        api = CfaitMobile(filesDir.absolutePath)
        
        // Preload data into memory immediately so UI is ready faster
        api.loadFromCache()
    }
}