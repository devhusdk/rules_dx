#import "greeter.h"

#include <string>

std::string DXNonArcGreeting() {
  DXGreeter *greeter = [[DXGreeter alloc] init];
  std::string out = [[greeter greet:@"non_arc"] UTF8String];
  [greeter release];
  return out;
}
