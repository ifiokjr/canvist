package io.github.ifiokjr.canvist

import android.view.KeyEvent

/** Keys as the editor's key bindings name them, after DOM `KeyboardEvent.key`. */
internal object KeyNames {
    private val special =
        mapOf(
            KeyEvent.KEYCODE_DEL to "Backspace",
            KeyEvent.KEYCODE_FORWARD_DEL to "Delete",
            KeyEvent.KEYCODE_ENTER to "Enter",
            KeyEvent.KEYCODE_NUMPAD_ENTER to "Enter",
            KeyEvent.KEYCODE_TAB to "Tab",
            KeyEvent.KEYCODE_DPAD_LEFT to "ArrowLeft",
            KeyEvent.KEYCODE_DPAD_RIGHT to "ArrowRight",
            KeyEvent.KEYCODE_DPAD_UP to "ArrowUp",
            KeyEvent.KEYCODE_DPAD_DOWN to "ArrowDown",
            KeyEvent.KEYCODE_MOVE_HOME to "Home",
            KeyEvent.KEYCODE_MOVE_END to "End",
            KeyEvent.KEYCODE_PAGE_UP to "PageUp",
            KeyEvent.KEYCODE_PAGE_DOWN to "PageDown",
            KeyEvent.KEYCODE_ESCAPE to "Escape",
        )

    /** The binding name of [event]'s key, or `null` for keys with no name. */
    fun of(event: KeyEvent): String? {
        special[event.keyCode]?.let { return it }

        // The key's own character: with Control or Meta held, the character
        // the modifiers would produce is none at all.
        val character = event.getUnicodeChar(event.metaState and MODIFIERS.inv())

        return if (character == 0) null else String(Character.toChars(character)).lowercase()
    }

    private const val MODIFIERS =
        KeyEvent.META_SHIFT_MASK or KeyEvent.META_CTRL_MASK or
            KeyEvent.META_META_MASK or KeyEvent.META_ALT_MASK

    fun modifiers(event: KeyEvent) =
        Modifiers(
            shift = event.isShiftPressed,
            ctrl = event.isCtrlPressed,
            alt = event.isAltPressed,
            meta = event.isMetaPressed,
        )
}
