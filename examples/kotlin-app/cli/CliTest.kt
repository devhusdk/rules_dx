package cli

import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayOutputStream
import java.io.PrintStream
import java.nio.file.Files

class CliTest {
    @get:Rule val temp = TemporaryFolder()

    private val out = ByteArrayOutputStream()
    private val err = ByteArrayOutputStream()

    private fun run(
        vararg args: String,
        env: Map<String, String> = emptyMap(),
    ): Int =
        Cli.run(
            args.toList(),
            env,
            PrintStream(out, true, Charsets.UTF_8),
            PrintStream(err, true, Charsets.UTF_8),
        )

    private fun stdout() = out.toString(Charsets.UTF_8).replace("\r\n", "\n")

    private fun stderr() = err.toString(Charsets.UTF_8).replace("\r\n", "\n")

    @Test
    fun listReadsTheBundledGrades() {
        assertEquals(Cli.OK, run("list"))
        assertEquals("ada 91.7 A\nbo 72.3 C\ncy 96.7 A\n", stdout())
    }

    @Test
    fun ranksAndLetters() {
        assertEquals(Cli.OK, run("ranks"))
        assertEquals("1 cy 96.7 A\n2 ada 91.7 A\n3 bo 72.3 C\n", stdout())
        out.reset()
        assertEquals(Cli.OK, run("letters"))
        assertEquals("A cy ada\nC bo\n", stdout())
    }

    @Test
    fun showPrintsScores() {
        assertEquals(Cli.OK, run("show", "ada"))
        assertEquals("ada 95 88 92\naverage 91.7\n", stdout())
    }

    @Test
    fun schemaComesFromTheGeneratedObject() {
        assertEquals(Cli.OK, run("schema"))
        assertEquals("schema 2\n", stdout())
    }

    @Test
    fun unknownStudentExits4() {
        assertEquals(Cli.NOT_FOUND, run("show", "zed"))
        assertEquals("grades: no student zed\n", stderr())
    }

    @Test
    fun badUsageExits2() {
        assertEquals(Cli.USAGE, run())
        assertEquals(Cli.USAGE, run("sell"))
        assertEquals(Cli.USAGE, run("show"))
        assertEquals(Cli.USAGE, run("--file"))
    }

    @Test
    fun fileFlagAndEnvSelectTheInput() {
        val file = temp.newFile("one.json").toPath()
        Files.writeString(file, """[{"name":"x","scores":[50]}]""")
        assertEquals(Cli.OK, run("--file", file.toString(), "list"))
        assertEquals("x 50.0 F\n", stdout())
        out.reset()
        assertEquals(Cli.OK, run("list", env = mapOf("GRADES_FILE" to file.toString())))
        assertEquals("x 50.0 F\n", stdout())
    }

    @Test
    fun relativeFileResolvesAgainstTheInvokingDirectory() {
        val file = temp.newFile("rel.json").toPath()
        Files.writeString(file, "[]")
        val env = mapOf("BUILD_WORKING_DIRECTORY" to temp.root.toString())
        assertEquals(Cli.OK, run("--file", "rel.json", "list", env = env))
        assertEquals("", stdout())
    }

    @Test
    fun missingFileExits3() {
        assertEquals(Cli.BAD_FILE, run("--file", "/nonexistent/x.json", "list"))
        assertEquals("grades: cannot read grade file\n", stderr())
    }

    @Test
    fun malformedFileExits3() {
        val file = temp.newFile("bad.json").toPath()
        Files.writeString(file, """[{"name":"x","scores":[101]}]""")
        assertEquals(Cli.BAD_FILE, run("--file", file.toString(), "list"))
        assertEquals("grades: score must be a whole number from 0 to 100\n", stderr())
    }
}
