#import "CanvistEditorsModule.h"

#import "CanvistSwift.h"

// Every method runs synchronously on the JavaScript thread. A failure raises
// an exception, which React Native rethrows in JavaScript with its reason:
// `[canvist:<code>] <message>`.
static id Checked(id result, NSError *error)
{
  if (result == nil) {
    @throw [NSException exceptionWithName:@"CanvistError" reason:error.localizedDescription userInfo:nil];
  }

  return result;
}

#define CANVIST_CALL(expression)               \
  ({                                           \
    NSError *error = nil;                      \
    id result = [CanvistHostCalls expression]; \
    Checked(result, error);                    \
  })

@implementation CanvistEditorsModule

+ (NSString *)moduleName
{
  return @"CanvistEditors";
}

- (std::shared_ptr<facebook::react::TurboModule>)getTurboModule:(const facebook::react::ObjCTurboModule::InitParams &)params
{
  return std::make_shared<facebook::react::NativeCanvistEditorsSpecJSI>(params);
}

- (NSNumber *)run:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  return CANVIST_CALL(runWithId:editorId name:name args:args error:&error);
}

- (NSNumber *)can:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  return CANVIST_CALL(canWithId:editorId name:name args:args error:&error);
}

- (NSString *)query:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  return CANVIST_CALL(queryWithId:editorId name:name args:args error:&error);
}

- (NSNumber *)load:(NSString *)editorId format:(NSString *)format content:(NSString *)content
{
  return CANVIST_CALL(loadWithId:editorId format:format content:content error:&error);
}

- (NSString *)exportDocument:(NSString *)editorId format:(NSString *)format
{
  return CANVIST_CALL(exportWithId:editorId format:format error:&error);
}

- (NSString *)selection:(NSString *)editorId
{
  return CANVIST_CALL(selectionWithId:editorId error:&error);
}

- (NSNumber *)select:(NSString *)editorId range:(NSString *)range
{
  return CANVIST_CALL(selectWithId:editorId range:range error:&error);
}

- (NSNumber *)selectAll:(NSString *)editorId
{
  return CANVIST_CALL(selectAllWithId:editorId error:&error);
}

- (NSNumber *)insertText:(NSString *)editorId text:(NSString *)text
{
  return CANVIST_CALL(insertTextWithId:editorId text:text error:&error);
}

- (NSNumber *)undo:(NSString *)editorId
{
  return CANVIST_CALL(undoWithId:editorId error:&error);
}

- (NSNumber *)redo:(NSString *)editorId
{
  return CANVIST_CALL(redoWithId:editorId error:&error);
}

- (NSNumber *)canUndo:(NSString *)editorId
{
  return CANVIST_CALL(canUndoWithId:editorId error:&error);
}

- (NSNumber *)canRedo:(NSString *)editorId
{
  return CANVIST_CALL(canRedoWithId:editorId error:&error);
}

@end
