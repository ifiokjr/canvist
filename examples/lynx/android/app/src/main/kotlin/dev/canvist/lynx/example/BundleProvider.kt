package dev.canvist.lynx.example

import android.content.Context
import com.lynx.tasm.provider.AbsTemplateProvider
import java.io.IOException
import kotlin.concurrent.thread

/** Reads Lynx bundles from the app's assets, off the main thread. */
class BundleProvider(
    context: Context,
) : AbsTemplateProvider() {
    private val assets = context.applicationContext.assets

    override fun loadTemplate(
        uri: String,
        callback: Callback,
    ) {
        thread(name = "lynx-bundle") {
            try {
                callback.onSuccess(assets.open(uri).use { it.readBytes() })
            } catch (error: IOException) {
                callback.onFailed(error.message)
            }
        }
    }
}
