package io.github.ifiokjr.canvist.reactnative

import com.facebook.react.BaseReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.model.ReactModuleInfo
import com.facebook.react.module.model.ReactModuleInfoProvider
import com.facebook.react.uimanager.ViewManager

/** canvist's editor view and the module that calls into its editors. */
class CanvistReactNativePackage : BaseReactPackage() {
    override fun createViewManagers(reactContext: ReactApplicationContext): List<ViewManager<*, *>> = listOf(CanvistEditorViewManager())

    override fun getModule(
        name: String,
        reactContext: ReactApplicationContext,
    ): NativeModule? = if (name == CanvistEditorsModule.NAME) CanvistEditorsModule(reactContext) else null

    override fun getReactModuleInfoProvider() =
        ReactModuleInfoProvider {
            mapOf(
                CanvistEditorsModule.NAME to
                    ReactModuleInfo(
                        name = CanvistEditorsModule.NAME,
                        className = CanvistEditorsModule.NAME,
                        canOverrideExistingModule = false,
                        needsEagerInit = false,
                        isCxxModule = false,
                        isTurboModule = true,
                    ),
            )
        }
}
