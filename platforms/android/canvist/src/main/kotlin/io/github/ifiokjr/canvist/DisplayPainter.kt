package io.github.ifiokjr.canvist

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF

/**
 * Paints display items on a [Canvas] with the paints the measurer shaped
 * text with, so every glyph lands where layout measured it. Items are in
 * dp; [origin] is where document (0, 0) is, in pixels.
 */
internal class DisplayPainter(
    private val fonts: FontTable,
) {
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val rect = RectF()
    private val path = Path()

    /** Colours for the parts of the page the theme does not colour. */
    var checkboxBorder = CHECKBOX_BORDER
    var checkboxFill = CHECKBOX_FILL
    var slotFill = SLOT_FILL

    /** Fill [area] with [page], then draw every item. */
    fun paint(
        canvas: Canvas,
        area: RectF,
        items: List<DisplayItem>,
        page: Rgba,
        originX: Float,
        originY: Float,
    ) {
        fill.color = page.argb()
        canvas.drawRect(area, fill)
        val density = fonts.density
        val saved = canvas.save()
        canvas.translate(originX, originY)

        for (item in items) draw(canvas, item, density)

        canvas.restoreToCount(saved)
    }

    private fun draw(
        canvas: Canvas,
        item: DisplayItem,
        density: Float,
    ) {
        when (item) {
            is DisplayItem.Fill -> fill(canvas, item.bounds, item.color.argb(), item.radius, density)
            is DisplayItem.Text -> text(canvas, item, density)
            is DisplayItem.Line -> line(canvas, item, density)
            is DisplayItem.Stroke -> ink(canvas, item, density)
            is DisplayItem.Checkbox -> checkbox(canvas, item.bounds, item.checked, density)
            is DisplayItem.Slot -> fill(canvas, item.bounds, slotFill, SLOT_RADIUS, density)
        }
    }

    private fun fill(
        canvas: Canvas,
        bounds: Bounds,
        color: Int,
        radius: Float,
        density: Float,
    ) {
        fill.color = color
        rect.set(bounds, density)

        if (radius > 0f) {
            canvas.drawRoundRect(rect, radius * density, radius * density, fill)
        } else {
            canvas.drawRect(rect, fill)
        }
    }

    private fun text(
        canvas: Canvas,
        run: DisplayItem.Text,
        density: Float,
    ) {
        val paint = fonts.paint(run.font) ?: return
        val color = paint.color
        val spacing = paint.letterSpacing
        paint.color = run.color.argb()
        paint.letterSpacing = if (run.letterSpacing == 0f) 0f else run.letterSpacing / (paint.textSize / density)

        val length = run.text.length
        canvas.drawTextRun(
            // text =
            run.text,
            // start =
            0,
            // end =
            length,
            // contextStart =
            0,
            // contextEnd =
            length,
            // x =
            run.x * density,
            // y =
            run.baseline * density,
            // isRtl =
            run.rtl,
            // paint =
            paint,
        )

        paint.color = color
        paint.letterSpacing = spacing
    }

    private fun line(
        canvas: Canvas,
        line: DisplayItem.Line,
        density: Float,
    ) {
        val left = line.x * density
        val y = line.y * density
        val width = line.width * density
        val thickness = line.thickness * density

        if (line.kind != LineKind.SQUIGGLE) {
            fill.color = line.color.argb()
            canvas.drawRect(left, y - thickness / 2, left + width, y + thickness / 2, fill)

            return
        }

        squiggle(left, y, width, thickness)
        stroke.color = line.color.argb()
        stroke.strokeWidth = thickness
        canvas.drawPath(path, stroke)
    }

    /**
     * An ink stroke: a polyline through the item's points, `x, y` pairs,
     * with round caps and joins. The first point is drawn to as well, so
     * one point leaves a dot.
     */
    private fun ink(
        canvas: Canvas,
        ink: DisplayItem.Stroke,
        density: Float,
    ) {
        val points = ink.points
        path.reset()

        for (index in 0 until points.size - 1 step 2) {
            val x = points[index] * density
            val y = points[index + 1] * density

            if (index == 0) path.moveTo(x, y)
            path.lineTo(x, y)
        }

        stroke.color = ink.color.argb()
        stroke.strokeWidth = ink.width * density
        stroke.strokeCap = Paint.Cap.ROUND
        stroke.strokeJoin = Paint.Join.ROUND
        canvas.drawPath(path, stroke)
    }

    /** A wavy line along [y] from [left], rising and falling in turn. */
    private fun squiggle(
        left: Float,
        y: Float,
        width: Float,
        thickness: Float,
    ) {
        val amplitude = thickness * SQUIGGLE_AMPLITUDE
        val half = amplitude * SQUIGGLE_WAVELENGTH / 2
        var x = left
        var up = true
        path.reset()
        path.moveTo(left, y)

        while (x < left + width) {
            val next = minOf(x + half, left + width)
            val peak = if (up) y - amplitude * 2 else y + amplitude * 2
            path.quadTo((x + next) / 2, peak, next, y)
            x = next
            up = !up
        }
    }

    private fun checkbox(
        canvas: Canvas,
        bounds: Bounds,
        checked: Boolean,
        density: Float,
    ) {
        val border = CHECKBOX_BORDER_WIDTH * density
        val radius = CHECKBOX_RADIUS * density
        rect.set(bounds, density)
        rect.inset(border / 2, border / 2)

        if (!checked) {
            stroke.color = checkboxBorder
            stroke.strokeWidth = border
            canvas.drawRoundRect(rect, radius, radius, stroke)

            return
        }

        fill.color = checkboxFill
        canvas.drawRoundRect(rect, radius, radius, fill)

        path.reset()
        TICK.forEachIndexed { index, (x, y) ->
            val pointX = rect.left + rect.width() * x
            val pointY = rect.top + rect.height() * y

            if (index == 0) path.moveTo(pointX, pointY) else path.lineTo(pointX, pointY)
        }

        stroke.color = TICK_COLOR
        stroke.strokeWidth = border
        stroke.strokeCap = Paint.Cap.ROUND
        stroke.strokeJoin = Paint.Join.ROUND
        canvas.drawPath(path, stroke)
    }

    private companion object {
        const val SLOT_RADIUS = 6f
        const val CHECKBOX_RADIUS = 3f
        const val CHECKBOX_BORDER_WIDTH = 1.5f
        const val SQUIGGLE_AMPLITUDE = 1.5f
        const val SQUIGGLE_WAVELENGTH = 4f
        const val CHECKBOX_BORDER = 0xFF8A8580.toInt()
        const val CHECKBOX_FILL = 0xFF6A00F5.toInt()
        const val SLOT_FILL = 0x1F787880
        const val TICK_COLOR = 0xFFFFFFFF.toInt()

        /** The check mark's corners, as fractions of the box. */
        val TICK = listOf(0.25f to 0.5f, 0.43f to 0.72f, 0.78f to 0.28f)
    }
}

/** A colour from the display list as an Android ARGB colour int. */
internal fun Rgba.argb(): Int = Color.argb(a.toInt(), r.toInt(), g.toInt(), b.toInt())

/** [bounds] in dp, scaled to pixels. */
internal fun RectF.set(
    bounds: Bounds,
    density: Float,
) {
    val x = bounds.x * density
    val y = bounds.y * density

    set(x, y, x + bounds.width * density, y + bounds.height * density)
}
