#import "CanvistLynxModule.h"

#import "CanvistSwift.h"

/// `text` as a JSON string literal.
static NSString *JSONString(NSString *text)
{
  NSData *data = [NSJSONSerialization dataWithJSONObject:text options:NSJSONWritingFragmentsAllowed error:nil];
  return [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
}

/// The envelope for `json`, a JSON value, or for `error` when the call failed.
static NSString *Reply(NSString *_Nullable json, NSError *_Nullable error)
{
  if (json == nil) {
    return [NSString stringWithFormat:@"{\"error\":%@}", JSONString(error.localizedDescription ?: @"[canvist:editor] unknown failure")];
  }

  return [NSString stringWithFormat:@"{\"value\":%@}", json];
}

static NSString *_Nullable JSONBool(NSNumber *_Nullable value)
{
  return value == nil ? nil : (value.boolValue ? @"true" : @"false");
}

static NSString *_Nullable JSONText(NSString *_Nullable value)
{
  return value == nil ? nil : JSONString(value);
}

@implementation CanvistLynxModule

+ (NSString *)name
{
  return @"CanvistEditors";
}

+ (NSDictionary<NSString *, NSString *> *)methodLookup
{
  return @{
    @"run" : NSStringFromSelector(@selector(run:name:args:)),
    @"can" : NSStringFromSelector(@selector(can:name:args:)),
    @"query" : NSStringFromSelector(@selector(query:name:args:)),
    @"load" : NSStringFromSelector(@selector(load:format:content:)),
    @"exportDocument" : NSStringFromSelector(@selector(exportDocument:format:)),
    @"selection" : NSStringFromSelector(@selector(selection:)),
    @"select" : NSStringFromSelector(@selector(select:range:)),
    @"selectAll" : NSStringFromSelector(@selector(selectAll:)),
    @"insertText" : NSStringFromSelector(@selector(insertText:text:)),
    @"undo" : NSStringFromSelector(@selector(undo:)),
    @"redo" : NSStringFromSelector(@selector(redo:)),
    @"canUndo" : NSStringFromSelector(@selector(canUndo:)),
    @"canRedo" : NSStringFromSelector(@selector(canRedo:)),
  };
}

- (NSString *)run:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls runWithId:editorId name:name args:args error:&error]), error);
}

- (NSString *)can:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls canWithId:editorId name:name args:args error:&error]), error);
}

- (NSString *)query:(NSString *)editorId name:(NSString *)name args:(NSString *)args
{
  NSError *error = nil;
  return Reply([CanvistHostCalls queryWithId:editorId name:name args:args error:&error], error);
}

- (NSString *)load:(NSString *)editorId format:(NSString *)format content:(NSString *)content
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls loadWithId:editorId format:format content:content error:&error]), error);
}

- (NSString *)exportDocument:(NSString *)editorId format:(NSString *)format
{
  NSError *error = nil;
  return Reply(JSONText([CanvistHostCalls exportWithId:editorId format:format error:&error]), error);
}

- (NSString *)selection:(NSString *)editorId
{
  NSError *error = nil;
  return Reply([CanvistHostCalls selectionWithId:editorId error:&error], error);
}

- (NSString *)select:(NSString *)editorId range:(NSString *)range
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls selectWithId:editorId range:range error:&error]), error);
}

- (NSString *)selectAll:(NSString *)editorId
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls selectAllWithId:editorId error:&error]), error);
}

- (NSString *)insertText:(NSString *)editorId text:(NSString *)text
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls insertTextWithId:editorId text:text error:&error]), error);
}

- (NSString *)undo:(NSString *)editorId
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls undoWithId:editorId error:&error]), error);
}

- (NSString *)redo:(NSString *)editorId
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls redoWithId:editorId error:&error]), error);
}

- (NSString *)canUndo:(NSString *)editorId
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls canUndoWithId:editorId error:&error]), error);
}

- (NSString *)canRedo:(NSString *)editorId
{
  NSError *error = nil;
  return Reply(JSONBool([CanvistHostCalls canRedoWithId:editorId error:&error]), error);
}

@end
