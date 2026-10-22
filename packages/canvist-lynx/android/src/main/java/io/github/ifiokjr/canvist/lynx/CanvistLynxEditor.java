package io.github.ifiokjr.canvist.lynx;

import android.content.Context;
import androidx.annotation.NonNull;
import com.lynx.react.bridge.Callback;
import com.lynx.react.bridge.ReadableArray;
import com.lynx.react.bridge.ReadableMap;
import com.lynx.tasm.behavior.LynxContext;
import com.lynx.tasm.behavior.LynxProp;
import com.lynx.tasm.behavior.LynxUIMethod;
import com.lynx.tasm.behavior.LynxUIMethodConstants;
import com.lynx.tasm.behavior.ui.LynxUI;
import com.lynx.tasm.event.LynxCustomEvent;
import io.github.ifiokjr.canvist.CanvistHostView;
import io.github.ifiokjr.canvist.EditorChange;
import java.util.ArrayList;
import java.util.List;

/**
 * {@code <canvist-editor>}: a {@link CanvistHostView} created from the
 * element's first attributes, sending {@code ready}, {@code change},
 * {@code focuschange}, and {@code error} events.
 */
public class CanvistLynxEditor extends LynxUI<CanvistHostView> implements CanvistHostView.Listener {
  private String editorId;
  private List<String> extensions = new ArrayList<>();
  private String contentFormat;
  private String content;

  public CanvistLynxEditor(LynxContext context) {
    super(context);
  }

  @Override
  protected CanvistHostView createView(Context context) {
    CanvistHostView host = new CanvistHostView(context);
    host.setListener(this);
    return host;
  }

  @LynxProp(name = "editor-id")
  public void setEditorId(String value) {
    editorId = value;
  }

  @LynxProp(name = "extensions")
  public void setExtensions(ReadableArray value) {
    List<String> names = new ArrayList<>();
    for (int index = 0; value != null && index < value.size(); index++) {
      names.add(value.getString(index));
    }
    extensions = names;
  }

  @LynxProp(name = "content-format")
  public void setContentFormat(String value) {
    contentFormat = value;
  }

  @LynxProp(name = "content")
  public void setContent(String value) {
    content = value;
  }

  @LynxProp(name = "text-size")
  public void setTextSize(float value) {
    mView.setFontSize(value);
  }

  /**
   * The editor starts once the first attributes have all arrived; later
   * changes to {@code extensions} and {@code content} have no effect, as the
   * JavaScript API documents.
   */
  @Override
  public void onPropsUpdated() {
    super.onPropsUpdated();

    if (editorId == null || editorId.isEmpty()) {
      return;
    }

    mView.start(editorId, extensions, emptyAsNull(contentFormat), emptyAsNull(content));
  }

  @Override
  public void destroy() {
    mView.stop();
    super.destroy();
  }

  @LynxUIMethod
  public void focus(ReadableMap params, Callback callback) {
    mView.focusEditor();
    callback.invoke(LynxUIMethodConstants.SUCCESS);
  }

  @LynxUIMethod
  public void blur(ReadableMap params, Callback callback) {
    mView.blurEditor();
    callback.invoke(LynxUIMethodConstants.SUCCESS);
  }

  @Override
  public void onReady(@NonNull CanvistHostView view, @NonNull String editorId) {
    LynxCustomEvent event = new LynxCustomEvent(getSign(), "ready");
    event.addDetail("editorId", editorId);
    emit(event);
  }

  @Override
  public void onChange(@NonNull CanvistHostView view, @NonNull EditorChange change) {
    LynxCustomEvent event = new LynxCustomEvent(getSign(), "change");
    event.addDetail("document", change.getDocument());
    event.addDetail("selection", change.getSelection());
    event.addDetail("composition", change.getComposition());
    emit(event);
  }

  @Override
  public void onFocusChange(@NonNull CanvistHostView view, boolean focused) {
    LynxCustomEvent event = new LynxCustomEvent(getSign(), "focuschange");
    event.addDetail("focused", focused);
    emit(event);
  }

  @Override
  public void onError(@NonNull CanvistHostView view, @NonNull String code, @NonNull String message) {
    LynxCustomEvent event = new LynxCustomEvent(getSign(), "error");
    event.addDetail("code", code);
    event.addDetail("message", message);
    emit(event);
  }

  private void emit(LynxCustomEvent event) {
    getLynxContext().getEventEmitter().sendCustomEvent(event);
  }

  private static String emptyAsNull(String value) {
    return value == null || value.isEmpty() ? null : value;
  }
}
