#import <Foundation/Foundation.h>

int tidy_dirty_objc_value(int value) {
  if (value) {
    return 1;
  } else {
    return 0;
  }
}
