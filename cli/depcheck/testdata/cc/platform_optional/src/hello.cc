#include "hello.h"
#include <greet/greet.h>
#include <optionalfeat/feat.h>

int Hello() { return greet::hello() + optionalfeat::label(); }
