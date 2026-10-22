#import <Foundation/Foundation.h>
#import <Lynx/LynxModule.h>

NS_ASSUME_NONNULL_BEGIN

/// The `CanvistEditors` native module: synchronous calls, on the JavaScript
/// thread, into the editors of `<canvist-editor>` elements. Every method
/// answers with a JSON envelope, `{"value": ...}` or `{"error": message}`,
/// because Lynx reports native exceptions to the host rather than to
/// JavaScript. Register it with `[config registerModule:CanvistLynxModule.class]`.
@interface CanvistLynxModule : NSObject <LynxModule>
@end

NS_ASSUME_NONNULL_END
