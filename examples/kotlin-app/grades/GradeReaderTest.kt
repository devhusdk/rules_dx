package grades

import java.io.StringReader
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class GradeReaderTest {
    private val one = """[{"name":"ada","scores":[90,80]}]"""

    private fun message(json: String): String =
        assertThrows(GradeException::class.java) { GradeReader.read(StringReader(json)) }.message!!

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
    fun rejectsBrokenJson() {
        assertEquals("grade file is not valid JSON", message("[{\"name\": "))
    }

    @Test
    fun rejectsNonObjectStudent() {
        assertEquals("student must be an object", message("[1]"))
    }
}
