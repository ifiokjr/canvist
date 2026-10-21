package io.github.ifiokjr.canvist

import android.graphics.Paint
import android.graphics.Typeface
import android.os.Build
import android.text.TextPaint

/**
 * Measures text for the editor with Android's text stack (`Paint`, the
 * engine `TextView` and `Canvas` draw with), so lines wrap where Android would
 * wrap them and glyphs land where they were measured.
 *
 * Layout works in density-independent pixels; text is shaped at [density]
 * physical pixels per dp, the size it is drawn at, and reported back in dp.
 * [CanvistView] makes one per editor. Hosts that drive an [Editor] without the
 * view (tests, React Native, Lynx) can give it one too.
 */
class PaintMeasurer(density: Float) : PlatformMeasurer {
    internal val fonts = FontTable(density)

    override fun measure(request: MeasureRequest): MeasureResponse {
        for (face in request.fonts) fonts.register(face)

        val density = fonts.density
        val metrics = request.metrics.map { id -> metrics(fonts.paint(id), density) }
        val advances = ArrayList<Float>(request.segments.sumOf { it.text.length })

        for (segment in request.segments) {
            advances(segment.text, fonts.paint(segment.font), density, advances)
        }

        return MeasureResponse(metrics, advances)
    }

    internal companion object {
        /** The extent of [paint]'s font around the baseline, in dp. */
        fun metrics(paint: TextPaint?, density: Float): VerticalMetrics {
            val font = paint?.fontMetrics ?: return VerticalMetrics(0f, 0f)

            return VerticalMetrics(-font.ascent / density, font.descent / density)
        }

        /**
         * Append the advance of each Unicode scalar of [text] in [paint], in dp:
         * what `Paint` advances the pen by for it, shaped in context, so the
         * advances always add up to the run's width.
         */
        fun advances(text: String, paint: TextPaint?, density: Float, out: MutableList<Float>) {
            val units = FloatArray(text.length)
            paint?.getTextRunAdvances(text.toCharArray(), 0, text.length, 0, text.length, isRtl(text), units, 0)
            var index = 0

            while (index < text.length) {
                val next = index + Character.charCount(text.codePointAt(index))
                out += units.sum(index, next) / density
                index = next
            }
        }

        /** The sum of [from] until [to]: a scalar's advance over its UTF-16 units. */
        private fun FloatArray.sum(from: Int, to: Int): Float {
            var total = 0f

            for (index in from until to) total += this[index]

            return total
        }

        /** Whether [text]'s first strong character is right to left. */
        private fun isRtl(text: String): Boolean {
            val strong = text.codePoints()
                .map { Character.getDirectionality(it).toInt() }
                .filter { it in STRONG }
                .findFirst()

            return strong.isPresent && strong.asInt != Character.DIRECTIONALITY_LEFT_TO_RIGHT.toInt()
        }

        private val STRONG = setOf(
            Character.DIRECTIONALITY_LEFT_TO_RIGHT.toInt(),
            Character.DIRECTIONALITY_RIGHT_TO_LEFT.toInt(),
            Character.DIRECTIONALITY_RIGHT_TO_LEFT_ARABIC.toInt(),
        )
    }
}

/**
 * The paints for the fonts layout uses, by the id it gave each, resolved once
 * from their CSS family lists. The measurer fills it and the painter reads it,
 * so text is drawn in exactly the font it was measured in.
 */
internal class FontTable(density: Float) {
    private val faces = HashMap<UInt, FontFace>()
    private val paints = HashMap<UInt, TextPaint>()

    /** Physical pixels per dp that text is shaped and drawn at. */
    @Volatile
    var density = density
        private set

    @Synchronized
    fun register(face: FontFace) {
        faces[face.id] = face
        paints[face.id] = paint(face)
    }

    @Synchronized
    fun paint(id: UInt): TextPaint? = paints[id]

    /** Shape at [density] from now on. The editor must measure again. */
    @Synchronized
    fun setDensity(density: Float) {
        this.density = density

        for ((id, face) in faces) paints[id]?.textSize = face.size * density
    }

    private fun paint(face: FontFace) = TextPaint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG).apply {
        typeface = typeface(face)
        textSize = face.size * density

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) hinting = Paint.HINTING_OFF
    }

    private companion object {
        /** The CSS weight from which faces before Android 9 use the bold style. */
        const val BOLD_WEIGHT = 600

        /** The first family in [FontFace.families] Android has, at the face's weight and slant. */
        fun typeface(face: FontFace): Typeface {
            val base = face.families.firstNotNullOfOrNull(::family) ?: Typeface.SANS_SERIF
            val weight = face.weight.toInt().coerceIn(1, 1000)

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) return Typeface.create(base, weight, face.italic)

            return Typeface.create(base, style(weight >= BOLD_WEIGHT, face.italic))
        }

        /** The typeface style before Android 9 had weights. */
        fun style(bold: Boolean, italic: Boolean): Int = when {
            bold && italic -> Typeface.BOLD_ITALIC
            bold -> Typeface.BOLD
            italic -> Typeface.ITALIC
            else -> Typeface.NORMAL
        }

        /** [name] as a typeface, mapping CSS generic families to Android's. */
        fun family(name: String): Typeface? = when (name.lowercase()) {
            "system-ui", "ui-sans-serif", "sans-serif", "-apple-system", "blinkmacsystemfont" -> Typeface.SANS_SERIF
            "ui-monospace", "monospace" -> Typeface.MONOSPACE
            "ui-serif", "serif" -> Typeface.SERIF
            else -> Typeface.create(name, Typeface.NORMAL).takeUnless { it == Typeface.DEFAULT }
        }
    }
}
