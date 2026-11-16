#import "greeter.h"

@implementation DXGreeter
- (NSString *)greet:(NSString *)name {
  return [NSString stringWithFormat:@"hello %@", name];
}
@end
