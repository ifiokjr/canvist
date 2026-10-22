#import <Lynx/LynxUI.h>

#import "CanvistSwift.h"

NS_ASSUME_NONNULL_BEGIN

/// `<canvist-editor>`: a `CanvistHostView` created from the element's first
/// attributes, sending `ready`, `change`, `focuschange`, and `error` events.
@interface CanvistLynxEditor : LynxUI <CanvistHostView *>
@end

NS_ASSUME_NONNULL_END
