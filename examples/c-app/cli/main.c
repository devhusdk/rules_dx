#include <stdio.h>

#include "examples/c-app/cli/cli.h"

int main(int argc, char **argv) { return cli_run(argc, argv, stdout, stderr); }
