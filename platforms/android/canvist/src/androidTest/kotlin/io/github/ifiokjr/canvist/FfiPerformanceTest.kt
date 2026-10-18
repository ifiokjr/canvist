package io.github.ifiokjr.canvist

import android.content.pm.ApplicationInfo
import android.os.Build
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Measures one Kotlin → Rust round trip with the same workload as
 * `crates/canvist_ffi/benches/round_trip.rs`, so the difference is the cost of
 * the UniFFI/JNA binding. Results are logged under the `CanvistPerf` tag; the
 * assertions only guard against a broken measurement, not a slow device.
 */
@RunWith(AndroidJUnit4::class)
class FfiPerformanceTest {
    @Test
    fun insertTextAndTextRoundTrips() {
        EditorHandle().use { warm ->
            for (offset in 0u until WARMUP) warm.insertText(offset, "a")
        }

        val inserts = DoubleArray(SAMPLES)
        val reads = DoubleArray(SAMPLES)
        for (sample in 0 until SAMPLES) {
            EditorHandle().use { editor ->
                var start = System.nanoTime()
                for (offset in 0u until BATCH) editor.insertText(offset, "a")
                inserts[sample] = (System.nanoTime() - start).toDouble() / BATCH.toDouble()

                start = System.nanoTime()
                for (call in 0u until BATCH) editor.text()
                reads[sample] = (System.nanoTime() - start).toDouble() / BATCH.toDouble()
                assertEquals(BATCH.toInt(), editor.text().length)
            }
        }

        report("insert_text", inserts)
        report("text", reads)
    }

    private fun report(name: String, values: DoubleArray) {
        values.sort()
        val median = values[values.size / 2]
        val p90 = values[values.size * 9 / 10]
        assertTrue(median > 0)
        val flags = InstrumentationRegistry.getInstrumentation().targetContext.applicationInfo.flags
        val build = if (flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) "debug" else "release"
        Log.i(TAG, "PERF kotlin($build, ${Build.SUPPORTED_ABIS[0]}) $name median=${median.toLong()}ns p90=${p90.toLong()}ns")
    }

    private companion object {
        const val TAG = "CanvistPerf"
        const val WARMUP = 1_000u
        const val SAMPLES = 200
        const val BATCH = 100u
    }
}
