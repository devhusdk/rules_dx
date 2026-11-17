#include <stdio.h>

#include "cc/tests/fixtures/c_app/cli/cli.h"

int main(int argc, char **argv) { return cli_run(argc, argv, stdout, stderr); }
