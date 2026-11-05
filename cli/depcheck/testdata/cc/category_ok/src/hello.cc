#include "hello.h"
#include <greet/greet.h>
#include <testhelper/helper.h>

int Hello() { return greet::hello() + testhelper::check(); }
