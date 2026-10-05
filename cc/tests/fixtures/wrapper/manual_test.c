#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const char* target = getenv("TEST_TARGET");
    printf("PASS manual public test ran: %s\n", target ? target : "(null)");
    return 0;
}