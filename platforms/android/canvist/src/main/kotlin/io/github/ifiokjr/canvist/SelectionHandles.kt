package io.github.ifiokjr.canvist

import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.view.View

/** Which end of the selection a handle drags: either end of a range, or the caret. */
internal enum class End { START, END, CARET }

/** A selection handle: its drawable, which end it drags, and the point it marks. */
internal class Handle(val drawable: Drawable, val end: End, val point: Pair<Float, Float>)

/** The theme's selection handle drawables, placed against carets. */
internal class SelectionHandles(
    private val left: Drawable?,
    private val right: Drawable?,
    private val middle: Drawable?,
) {
    /** [end]'s handle under [caret], in content pixels, or `null` when the theme has none. */
    fun place(end: End, view: CanvistView, caret: Bounds): Pair<Handle, Rect>? {
        val drawable = drawable(end) ?: return null
        val rect = view.contentRect(caret)
        val width = drawable.intrinsicWidth

        // The start handle points up from its top-right corner, the end
        // handle from its top-left, the caret's from its middle.
        val hotspot = when (end) {
            End.START -> START_HOTSPOT
            End.END -> END_HOTSPOT
            End.CARET -> CARET_HOTSPOT
        }

        val x = (rect.centerX() - width * hotspot).toInt()
        val bounds = Rect(x, rect.bottom, x + width, rect.bottom + drawable.intrinsicHeight)

        return Handle(drawable, end, rect.exactCenterX() to rect.exactCenterY()) to bounds
    }

    private fun drawable(end: End) = when (end) {
        End.START -> left
        End.END -> right
        End.CARET -> middle
    }

    companion object {
        /** Where each handle's pointer is, as a fraction of its width. */
        const val START_HOTSPOT = 0.75f
        const val END_HOTSPOT = 0.25f
        const val CARET_HOTSPOT = 0.5f

        /** The handles [view]'s theme draws for text selection. */
        fun from(view: View): SelectionHandles {
            val attributes = view.context.obtainStyledAttributes(
                intArrayOf(
                    android.R.attr.textSelectHandleLeft,
                    android.R.attr.textSelectHandleRight,
                    android.R.attr.textSelectHandle,
                ),
            )
            val handles = SelectionHandles(
                left = attributes.getDrawable(0),
                right = attributes.getDrawable(1),
                middle = attributes.getDrawable(2),
            )
            attributes.recycle()

            return handles
        }
    }
}
