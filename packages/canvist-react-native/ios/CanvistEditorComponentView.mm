#import "CanvistEditorComponentView.h"

#import <react/renderer/components/CanvistReactNativeSpec/ComponentDescriptors.h>
#import <react/renderer/components/CanvistReactNativeSpec/EventEmitters.h>
#import <react/renderer/components/CanvistReactNativeSpec/Props.h>
#import <react/renderer/components/CanvistReactNativeSpec/RCTComponentViewHelpers.h>

#import "CanvistSwift.h"

using namespace facebook::react;

@interface CanvistEditorComponentView () <RCTCanvistEditorViewViewProtocol>
@end

@implementation CanvistEditorComponentView {
  CanvistHostView *_host;
}

+ (ComponentDescriptorProvider)componentDescriptorProvider
{
  return concreteComponentDescriptorProvider<CanvistEditorViewComponentDescriptor>();
}

// Each view owns one editor for its whole life, created from its first props.
+ (BOOL)shouldBeRecycled
{
  return NO;
}

- (instancetype)initWithFrame:(CGRect)frame
{
  if (self = [super initWithFrame:frame]) {
    static const auto defaultProps = std::make_shared<const CanvistEditorViewProps>();
    _props = defaultProps;
    _host = [CanvistHostView new];
    self.contentView = _host;

    __weak CanvistEditorComponentView *weakSelf = self;
    _host.onReady = ^(NSString *editorId) {
      if (auto emitter = [weakSelf editorEventEmitter]) {
        emitter->onEditorReady({.editorId = std::string(editorId.UTF8String)});
      }
    };
    _host.onChange = ^(BOOL document, BOOL selection, BOOL composition) {
      if (auto emitter = [weakSelf editorEventEmitter]) {
        emitter->onEditorChange({.document = (bool)document, .selection = (bool)selection, .composition = (bool)composition});
      }
    };
    _host.onFocusChange = ^(BOOL focused) {
      if (auto emitter = [weakSelf editorEventEmitter]) {
        emitter->onEditorFocus({.focused = (bool)focused});
      }
    };
    _host.onError = ^(NSString *code, NSString *message) {
      if (auto emitter = [weakSelf editorEventEmitter]) {
        emitter->onEditorError({.code = std::string(code.UTF8String), .message = std::string(message.UTF8String)});
      }
    };
  }

  return self;
}

- (void)dealloc
{
  [_host stop];
}

- (std::shared_ptr<const CanvistEditorViewEventEmitter>)editorEventEmitter
{
  return std::static_pointer_cast<const CanvistEditorViewEventEmitter>(_eventEmitter);
}

- (void)updateProps:(Props::Shared const &)props oldProps:(Props::Shared const &)oldProps
{
  const auto &oldViewProps = *std::static_pointer_cast<const CanvistEditorViewProps>(_props);
  const auto &newViewProps = *std::static_pointer_cast<const CanvistEditorViewProps>(props);

  if (oldViewProps.fontSize != newViewProps.fontSize) {
    _host.fontSize = newViewProps.fontSize;
  }

  if (oldViewProps.accessibilityLabel != newViewProps.accessibilityLabel) {
    _host.accessibilityLabel = newViewProps.accessibilityLabel.empty()
        ? nil
        : [NSString stringWithUTF8String:newViewProps.accessibilityLabel.c_str()];
  }

  [super updateProps:props oldProps:oldProps];
}

// The editor starts once the first transaction has mounted, when the event
// emitter that announces it exists.
- (void)finalizeUpdates:(RNComponentViewUpdateMask)updateMask
{
  [super finalizeUpdates:updateMask];

  const auto &props = *std::static_pointer_cast<const CanvistEditorViewProps>(_props);

  if (props.editorId.empty()) {
    return;
  }

  NSMutableArray<NSString *> *extensions = [NSMutableArray arrayWithCapacity:props.extensions.size()];
  for (const auto &name : props.extensions) {
    [extensions addObject:[NSString stringWithUTF8String:name.c_str()]];
  }

  NSString *format = props.contentFormat.empty() ? nil : [NSString stringWithUTF8String:props.contentFormat.c_str()];
  NSString *content = props.content.empty() ? nil : [NSString stringWithUTF8String:props.content.c_str()];
  [_host startWithEditorId:[NSString stringWithUTF8String:props.editorId.c_str()]
                extensions:extensions
                    format:format
                   content:content];
}

- (void)handleCommand:(const NSString *)commandName args:(const NSArray *)args
{
  RCTCanvistEditorViewHandleCommand(self, commandName, args);
}

- (void)focus
{
  [_host focus];
}

- (void)blur
{
  [_host blur];
}

@end
