package io.github.ifiokjr.canvist.lynx;

import com.lynx.tasm.LynxEnv;
import com.lynx.tasm.behavior.Behavior;
import com.lynx.tasm.behavior.LynxContext;
import com.lynx.tasm.behavior.ui.LynxUI;

/**
 * Registers canvist with Lynx: call {@link #registerModule()} after
 * {@code LynxEnv.inst().init(...)}, and add {@link #behavior()} to each
 * {@code LynxViewBuilder} with {@code addBehavior}.
 */
public final class CanvistLynx {
  /** The element's tag. */
  public static final String TAG = "canvist-editor";

  private CanvistLynx() {}

  /** Register the {@code CanvistEditors} native module with every LynxView. */
  public static void registerModule() {
    LynxEnv.inst().registerModule(CanvistLynxModule.NAME, CanvistLynxModule.class);
  }

  /** The behavior that creates {@code <canvist-editor>} elements. */
  public static Behavior behavior() {
    return new Behavior(TAG) {
      @Override
      public LynxUI createUI(LynxContext context) {
        return new CanvistLynxEditor(context);
      }
    };
  }
}
