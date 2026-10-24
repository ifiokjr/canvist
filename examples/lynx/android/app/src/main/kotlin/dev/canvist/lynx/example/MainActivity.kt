package dev.canvist.lynx.example

import android.app.Activity
import android.os.Bundle
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.lynx.tasm.LynxViewBuilder
import io.github.ifiokjr.canvist.lynx.CanvistLynx
import org.json.JSONObject

/**
 * Shows `main.lynx.bundle`, the example's ReactLynx app, between the system
 * bars and above the keyboard. Lynx knows neither, so the host pads its view.
 */
class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val lynxView =
            LynxViewBuilder()
                .setTemplateProvider(BundleProvider(this))
                .addBehavior(CanvistLynx.behavior())
                .build(this)
        val container = FrameLayout(this)
        container.addView(lynxView)
        ViewCompat.setOnApplyWindowInsetsListener(container) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime())
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        setContentView(container)
        // `perfSlack` multiplies the benchmark's budgets on slow CI emulators;
        // Maestro passes it as an intent extra.
        val initData =
            intent.extras
                ?.getString("perfSlack")
                ?.toDoubleOrNull()
                ?.let { JSONObject().put("perfSlack", it).toString() }
        lynxView.renderTemplateUrl("main.lynx.bundle", initData ?: "")
    }
}
