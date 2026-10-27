#import "greeter.h"

#include <string>

std::string DXLegacyGreeting() {
  DXGreeter *greeter = [[DXGreeter alloc] init];
  std::string out = [[greeter greet:@"legacy"] UTF8String];
  [greeter release];
  return out;
}
