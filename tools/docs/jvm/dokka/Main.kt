package dx.docs.dokka

import dx.docs.jvm.ApiRecordWriter
import java.io.File
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.system.exitProcess
import org.jetbrains.dokka.DokkaConfiguration
import org.jetbrains.dokka.DokkaConfigurationImpl
import org.jetbrains.dokka.DokkaGenerator
import org.jetbrains.dokka.DokkaSourceSetID
import org.jetbrains.dokka.DokkaSourceSetImpl
import org.jetbrains.dokka.Platform
import org.jetbrains.dokka.utilities.DokkaLogger

/** Runs Dokka in-process with the IR plugin and writes one API record file. */
object Main {
    private const val TOOL_VERSION = "2.2.0"

    /** Entry point; exits 1 and writes no output when the sources cannot be documented. */
    @JvmStatic
    fun main(args: Array<String>) {
        exitProcess(
            try {
                run(args)
            } catch (error: Exception) {
                System.err.println("dokka_extract: ${error.message ?: error}")
                1
            }
        )
    }

    private fun run(args: Array<String>): Int {
        val flags = LinkedHashMap<String, String>()
        var index = 0
        while (index < args.size) {
            if (index + 1 >= args.size) {
                return usage("${args[index]} needs a value")
            }
            if (args[index] !in FLAGS) {
                return usage("unknown argument '${args[index]}'")
            }
            flags[args[index]] = args[index + 1]
            index += 2
        }
        val missing = FLAGS.filter { it !in flags }
        if (missing.isNotEmpty()) {
            return usage("${FLAGS.joinToString(", ")} are required")
        }
        val sources = ArrayList<String>()
        val manifestLines = readLines(flags.getValue("--srcs-manifest"))
        for (line in manifestLines) {
            if (line.isEmpty()) {
                continue
            }
            val source = line.split('\t')[0]
            if (!Files.isRegularFile(Paths.get(source))) {
                System.err.println("dokka_extract: missing source $source")
                return 1
            }
            sources.add(source)
        }
        if (sources.isEmpty()) {
            System.err.println("dokka_extract: the srcs manifest lists no sources")
            return 1
        }
        val classpath = ArrayList<File>()
        val classpathLines = readLines(flags.getValue("--classpath-file"))
        for (line in classpathLines) {
            if (line.isEmpty()) {
                continue
            }
            if (!Files.exists(Paths.get(line))) {
                System.err.println("dokka_extract: missing classpath entry $line")
                return 1
            }
            classpath.add(File(line).absoluteFile)
        }
        val out = Paths.get(flags.getValue("--out"))
        Files.deleteIfExists(out)
        val scratch = Files.createTempDirectory("dx-dokka")
        try {
            return generate(sources, classpath, flags.getValue("--release"), out, scratch)
        } finally {
            scratch.toFile().deleteRecursively()
        }
    }

    private fun generate(
        sources: List<String>,
        classpath: List<File>,
        release: String,
        out: Path,
        scratch: Path,
    ): Int {
        val sourceSet =
            DokkaSourceSetImpl(
                displayName = "main",
                sourceSetID = DokkaSourceSetID("dx", "main"),
                classpath = classpath,
                sourceRoots = sources.map { File(it).absoluteFile }.toSet(),
                jdkVersion = release.toIntOrNull() ?: DEFAULT_JDK,
                noStdlibLink = true,
                noJdkLink = true,
                analysisPlatform = Platform.jvm,
                documentedVisibilities = setOf(DokkaConfiguration.Visibility.PUBLIC),
            )
        val configuration =
            DokkaConfigurationImpl(
                moduleName = "dx",
                outputDir = scratch.resolve("out").toFile(),
                offlineMode = true,
                sourceSets = listOf(sourceSet),
                suppressObviousFunctions = true,
                suppressInheritedMembers = true,
            )
        val logger = Logger()
        Job.files = SourceMap(sources)
        try {
            DokkaGenerator(configuration, logger).generate()
        } catch (error: Throwable) {
            System.err.println("dokka_extract: Dokka failed: $error")
            var cause = error.cause
            while (cause != null) {
                System.err.println("dokka_extract: caused by $cause")
                cause.stackTrace.take(STACK_LINES).forEach { System.err.println("    at $it") }
                cause = cause.cause
            }
            return 1
        }
        val symbols = Job.symbols
        if (logger.errorsCount > 0 || Job.problems.isNotEmpty() || symbols == null) {
            Job.problems.forEach { System.err.println("dokka_extract: $it") }
            System.err.println("dokka_extract: the sources could not be documented")
            return 1
        }
        ApiRecordWriter.write(out, "kotlin", "dokka", TOOL_VERSION, symbols)
        return 0
    }

    private fun readLines(path: String): List<String> =
        Files.readAllLines(Paths.get(path), StandardCharsets.UTF_8)

    private fun usage(message: String): Int {
        System.err.println(
            "dokka_extract: $message\nusage: dokka_extract --srcs-manifest FILE --classpath-file FILE" +
                " --release N --out FILE"
        )
        return 2
    }

    private const val DEFAULT_JDK = 17
    private const val STACK_LINES = 12
    private val FLAGS = listOf("--srcs-manifest", "--classpath-file", "--release", "--out")

    /** Collects Dokka's messages; warnings and errors go to stderr, errors fail the run. */
    private class Logger : DokkaLogger {
        override var warningsCount: Int = 0
        override var errorsCount: Int = 0

        override fun progress(message: String) = Unit

        override fun debug(message: String) = Unit

        override fun info(message: String) = Unit

        override fun warn(message: String) {
            warningsCount++
            System.err.println("dokka: warning: $message")
        }

        override fun error(message: String) {
            errorsCount++
            System.err.println("dokka: error: $message")
        }
    }
}
