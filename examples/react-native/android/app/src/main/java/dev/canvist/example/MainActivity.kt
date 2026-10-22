package dev.canvist.example

import android.os.Bundle
import com.facebook.react.ReactActivity
import com.facebook.react.ReactActivityDelegate
import com.facebook.react.defaults.DefaultNewArchitectureEntryPoint.fabricEnabled
import com.facebook.react.defaults.DefaultReactActivityDelegate

class MainActivity : ReactActivity() {
  override fun getMainComponentName(): String = "CanvistExample"

  override fun createReactActivityDelegate(): ReactActivityDelegate =
      object : DefaultReactActivityDelegate(this, mainComponentName, fabricEnabled) {
        /**
         * `perfSlack` multiplies the benchmark's budgets on slow CI emulators;
         * Maestro passes it as an intent extra.
         */
        override fun getLaunchOptions(): Bundle? {
          val slack = intent.extras?.getString("perfSlack")?.toDoubleOrNull() ?: return null
          return Bundle().apply { putDouble("perfSlack", slack) }
        }
      }
}
