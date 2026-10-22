package dev.canvist.lynx.example

import android.app.Application
import com.lynx.tasm.LynxEnv
import io.github.ifiokjr.canvist.lynx.CanvistLynx

/** Starts Lynx with the `CanvistEditors` module, for every LynxView. */
class ExampleApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        LynxEnv.inst().init(this, null, null, null)
        CanvistLynx.registerModule()
    }
}
