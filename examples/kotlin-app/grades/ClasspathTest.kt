package grades

import com.google.common.collect.ImmutableList
import com.google.gson.JsonParser
import java.nio.file.Path
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ClasspathTest {
    private fun jarOf(type: Class<*>): Path =
        Paths.get(type.protectionDomain.codeSource.location.toURI())

    @Test
    fun ownedClassesComeFromTheGradesJar() {
        assertTrue(jarOf(Student::class.java).fileName.toString().startsWith("grades_upstream"))
        assertEquals(jarOf(Student::class.java), jarOf(GradeReader::class.java))
        assertEquals(jarOf(Student::class.java), jarOf(BuildInfo::class.java))
    }

    @Test
    fun dependencyClassesComeFromTheirOwnJars() {
        assertNotEquals(jarOf(Student::class.java), jarOf(JsonParser::class.java))
        assertNotEquals(jarOf(Student::class.java), jarOf(ImmutableList::class.java))
        assertTrue(jarOf(JsonParser::class.java).fileName.toString().contains("gson"))
        assertTrue(jarOf(ImmutableList::class.java).fileName.toString().contains("guava"))
    }
}
