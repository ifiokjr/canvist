#import "CanvistLynxEditor.h"

#import <Lynx/LynxComponentRegistry.h>
#import <Lynx/LynxPropsProcessor.h>
#import <Lynx/LynxUIMethodProcessor.h>

#import "CanvistSwift.h"

@implementation CanvistLynxEditor {
  NSString *_editorId;
  NSArray<NSString *> *_extensions;
  NSString *_contentFormat;
  NSString *_content;
}

LYNX_LAZY_REGISTER_UI("canvist-editor")

- (CanvistHostView *)createView
{
  CanvistHostView *host = [CanvistHostView new];
  __weak CanvistLynxEditor *weakSelf = self;
  host.onReady = ^(NSString *editorId) {
    [weakSelf emit:@"ready" detail:@{@"editorId" : editorId}];
  };
  host.onChange = ^(BOOL document, BOOL selection, BOOL composition) {
    [weakSelf emit:@"change" detail:@{@"document" : @(document), @"selection" : @(selection), @"composition" : @(composition)}];
  };
  host.onFocusChange = ^(BOOL focused) {
    [weakSelf emit:@"focuschange" detail:@{@"focused" : @(focused)}];
  };
  host.onError = ^(NSString *code, NSString *message) {
    [weakSelf emit:@"error" detail:@{@"code" : code, @"message" : message}];
  };

  return host;
}

- (void)dealloc
{
  [self.view stop];
}

LYNX_PROP_SETTER("editor-id", setEditorId, NSString *)
{
  _editorId = value;
}

LYNX_PROP_SETTER("extensions", setExtensions, NSArray *)
{
  NSMutableArray<NSString *> *names = [NSMutableArray arrayWithCapacity:value.count];
  for (id name in value) {
    if ([name isKindOfClass:NSString.class]) {
      [names addObject:name];
    }
  }
  _extensions = names;
}

LYNX_PROP_SETTER("content-format", setContentFormat, NSString *)
{
  _contentFormat = value;
}

LYNX_PROP_SETTER("content", setContent, NSString *)
{
  _content = value;
}

LYNX_PROP_SETTER("text-size", setTextSize, CGFloat)
{
  self.view.fontSize = value;
}

// The editor starts once the first attributes have all arrived; later
// changes to `extensions` and `content` have no effect, as the JavaScript API
// documents.
- (void)propsDidUpdate
{
  [super propsDidUpdate];

  if (_editorId.length == 0) {
    return;
  }

  [self.view startWithEditorId:_editorId
                    extensions:_extensions ?: @[]
                        format:_contentFormat.length > 0 ? _contentFormat : nil
                       content:_content.length > 0 ? _content : nil];
}

LYNX_UI_METHOD(focus)
{
  [self.view focus];
  callback(kUIMethodSuccess, nil);
}

LYNX_UI_METHOD(blur)
{
  [self.view blur];
  callback(kUIMethodSuccess, nil);
}

- (void)emit:(NSString *)name detail:(NSDictionary *)detail
{
  LynxDetailEvent *event = [[LynxDetailEvent alloc] initWithName:name targetSign:[self sign] detail:detail];
  [self.context.eventEmitter dispatchCustomEvent:event];
}

@end
