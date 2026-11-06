package cli

import kotlin.system.exitProcess

/** Runs the grades command and exits with its code. */
fun main(args: Array<String>) {
    exitProcess(Cli.run(args.toList(), System.getenv(), System.out, System.err))
}
