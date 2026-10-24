package cli

import grades.BuildInfo
import grades.GradeException
import grades.GradeReader
import grades.Stats
import grades.Student
import java.io.IOException
import java.io.PrintStream
import java.nio.file.Files
import java.nio.file.Paths

/** Command line front end for the grade library. */
object Cli {
    /** Success. */
    const val OK = 0

    /** Bad command line. */
    const val USAGE = 2

    /** Unreadable or malformed grade file. */
    const val BAD_FILE = 3

    /** Unknown student. */
    const val NOT_FOUND = 4

    private const val USAGE_TEXT =
        "usage: grades [--file FILE] list | ranks | letters | schema | show NAME"

    /** Runs one command and returns its exit code. */
    fun run(
        args: List<String>,
        env: Map<String, String>,
        out: PrintStream,
        err: PrintStream,
    ): Int {
        var file = env["GRADES_FILE"]
        var rest = args
        if (rest.size >= 2 && rest[0] == "--file") {
            file = rest[1]
            rest = rest.drop(2)
        }
        val command = rest.firstOrNull()
        val operands = rest.drop(1)
        val known =
            when (command) {
                "list",
                "ranks",
                "letters",
                "schema",
                -> operands.isEmpty()

                "show" -> operands.size == 1

                else -> false
            }
        if (!known) {
            err.println(USAGE_TEXT)
            return USAGE
        }
        if (command == "schema") {
            out.println("schema ${BuildInfo.SCHEMA}")
            return OK
        }
        val students =
            try {
                load(file, env["BUILD_WORKING_DIRECTORY"])
            } catch (e: IOException) {
                err.println("grades: cannot read grade file")
                return BAD_FILE
            } catch (e: GradeException) {
                err.println("grades: ${e.message}")
                return BAD_FILE
            }
        return when (command) {
            "list" -> {
                students.forEach { out.println(describe(it)) }
                OK
            }

            "ranks" -> {
                Stats.ranked(students).forEachIndexed { i, s ->
                    out.println("${i + 1} ${describe(s)}")
                }
                OK
            }

            "letters" -> {
                val grouped = Stats.byLetter(students)
                grouped.keySet().sorted().forEach { letter ->
                    out.println("$letter ${grouped[letter].joinToString(" ") { it.name }}")
                }
                OK
            }

            else -> {
                show(students, operands[0], out, err)
            }
        }
    }

    private fun describe(student: Student): String {
        val average = student.averageTenths()
        return "${student.name} ${Stats.tenths(average)} ${Stats.letter(average)}"
    }

    private fun show(
        students: List<Student>,
        name: String,
        out: PrintStream,
        err: PrintStream,
    ): Int {
        val student = students.firstOrNull { it.name == name }
        if (student == null) {
            err.println("grades: no student $name")
            return NOT_FOUND
        }
        out.println("${student.name} ${student.scores.joinToString(" ")}")
        out.println("average ${Stats.tenths(student.averageTenths())}")
        return OK
    }

    private fun load(
        file: String?,
        workingDir: String?,
    ): List<Student> {
        if (file == null) {
            return GradeReader.bundled()
        }
        var path = Paths.get(file)
        if (!path.isAbsolute && workingDir != null) {
            path = Paths.get(workingDir).resolve(path)
        }
        return Files.newBufferedReader(path).use { GradeReader.read(it) }
    }
}
