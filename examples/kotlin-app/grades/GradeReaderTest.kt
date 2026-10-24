package grades

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.io.IOException
import java.io.StringReader

class GradeReaderTest {
    private val one = """[{"name":"ada","scores":[90,80]}]"""

    private fun message(json: String): String = assertThrows(GradeException::class.java) { GradeReader.read(StringReader(json)) }.message!!

    @Test
    fun bundledResourceIsLoadedFromTheLibrary() {
        val students = GradeReader.bundled()
        assertEquals(listOf("ada", "bo", "cy"), students.map { it.name })
        assertEquals(listOf(95, 88, 92), students[0].scores)
    }

    @Test
    fun readsOneStudent() {
        val students = GradeReader.read(StringReader(one))
        assertEquals(Student("ada", listOf(90, 80)), students.single())
        assertEquals(850, students.single().averageTenths())
    }

    @Test
    fun generatedBuildInfoCarriesTheSchemaFile() {
        assertEquals(2, BuildInfo.SCHEMA)
    }

    @Test
    fun rejectsScoreAboveMax() {
        assertEquals(
            "score must be a whole number from 0 to 100",
            message(one.replace("90", "101")),
        )
    }

    @Test
    fun rejectsNegativeScore() {
        assertEquals(
            "score must be a whole number from 0 to 100",
            message(one.replace("90", "-1")),
        )
    }

    @Test
    fun rejectsFractionalScore() {
        assertEquals(
            "score must be a whole number from 0 to 100",
            message(one.replace("90", "90.5")),
        )
    }

    @Test
    fun rejectsWrongType() {
        assertEquals("score must be a number", message(one.replace("90", "\"90\"")))
    }

    @Test
    fun rejectsEmptyScores() {
        assertEquals("scores must not be empty", message("""[{"name":"ada","scores":[]}]"""))
    }

    @Test
    fun rejectsNonArray() {
        assertEquals("grade file must hold an array", message("{}"))
    }

    @Test
    fun missingResourceIsReported() {
        val e =
            assertThrows(IOException::class.java) {
                GradeReader.fromResource("/grades/absent.json")
            }
        assertEquals("missing resource /grades/absent.json", e.message)
    }

    @Test
    fun rejectsNonStringName() {
        assertEquals("name must be a string", message("""[{"name":3,"scores":[1]}]"""))
    }

    @Test
    fun rejectsScoresThatAreNotAnArray() {
        assertEquals("scores must be an array", message("""[{"name":"ada","scores":{}}]"""))
    }

    @Test
    fun rejectsJsonThatOnlyALenientParserAccepts() {
        val lenient =
            listOf(
                "[{'name':'ada','scores':[1]}]",
                "[{name:\"ada\",scores:[1]}]",
                "// note\n$one",
                "$one []",
                """[{"name":"ada","scores":[1]},]""",
                "",
            )
        for (json in lenient) {
            assertEquals(json, "grade file is not valid JSON", message(json))
        }
    }

    @Test
    fun rejectsBrokenJson() {
        assertEquals("grade file is not valid JSON", message("[{\"name\": "))
    }

    @Test
    fun rejectsNonObjectStudent() {
        assertEquals("student must be an object", message("[1]"))
    }
}
