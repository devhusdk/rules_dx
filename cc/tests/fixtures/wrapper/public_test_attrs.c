#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int check(const char* label, const char* got, const char* want) {
    if (got == NULL || strcmp(got, want) != 0) {
        printf("FAIL %s: got [%s] want [%s]\n", label, got ? got : "(null)", want);
        return 1;
    }
    printf("PASS %s: [%s]\n", label, got);
    return 0;
}

static int shard_failures = 0;

static void announce_sharding(void) {
    const char* status = getenv("TEST_SHARD_STATUS_FILE");
    if (status == NULL) {
        printf("PASS sharding: TEST_SHARD_STATUS_FILE is unset\n");
        return;
    }
    FILE* f = fopen(status, "a");
    if (f == NULL) {
        printf("FAIL sharding: cannot write %s\n", status);
        shard_failures = 1;
        return;
    }
    fputs("1\n", f);
    fclose(f);
    printf("PASS sharding: wrote %s\n", status);
}

int main(void) {
    int failures = 0;
    failures += check("env", getenv("DX_PUBLIC_TEST_ENV"), "declared-value");
    failures += check("timeout", getenv("TEST_TIMEOUT"), "300");
    failures += check("shard count", getenv("TEST_TOTAL_SHARDS"), "2");
    const char* index = getenv("TEST_SHARD_INDEX");
    if (index == NULL || (strcmp(index, "0") != 0 && strcmp(index, "1") != 0)) {
        printf("FAIL shard index: got [%s]\n", index ? index : "(null)");
        failures += 1;
    } else {
        printf("PASS shard index: [%s]\n", index);
    }
    announce_sharding();
    return failures + shard_failures == 0 ? 0 : 1;
}
