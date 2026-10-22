package grades

import com.google.gson.Gson
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.stream.JsonReader
import java.io.IOException
import java.io.InputStreamReader
import java.io.Reader

/** Reads students from JSON. */
object GradeReader {
    /** Highest accepted score. */
    const val MAX = 100

    private val GSON = Gson()

    private const val BUNDLED = "/grades/grades.json"

    /** Reads the students packaged with this library. */
    fun bundled(): List<Student> = fromResource(BUNDLED)

    /** Reads the students stored in a classpath resource. */
    fun fromResource(resource: String): List<Student> {
        val stream =
            GradeReader::class.java.getResourceAsStream(resource)
                ?: throw IOException("missing resource $resource")
        return stream.use { read(InputStreamReader(it, Charsets.UTF_8)) }
    }

    /** Parses a JSON array of students. */
    fun read(reader: Reader): List<Student> {
        val root = parse(reader)
        if (!root.isJsonArray) {
            throw GradeException("grade file must hold an array")
        }
        return root.asJsonArray.map { student(obj(it, "student")) }
    }

    private fun parse(reader: Reader): JsonElement {
        try {
            JsonReader(reader).use { json ->
                json.isLenient = false
                val root = GSON.getAdapter(JsonElement::class.java).read(json)
                json.peek()
                return root
            }
        } catch (e: IOException) {
            throw GradeException("grade file is not valid JSON", e)
        }
    }

    private fun student(json: JsonObject): Student {
        val name = json.get("name")
        if (name == null || !name.isJsonPrimitive || !name.asJsonPrimitive.isString) {
            throw GradeException("name must be a string")
        }
        val scores = json.get("scores")
        if (scores == null || !scores.isJsonArray) {
            throw GradeException("scores must be an array")
        }
        try {
            return Student(name.asString, scores.asJsonArray.map { score(it) })
        } catch (e: IllegalArgumentException) {
            throw GradeException(e.message ?: "invalid student", e)
        }
    }

    private fun score(element: JsonElement): Int {
        if (!element.isJsonPrimitive || !element.asJsonPrimitive.isNumber) {
            throw GradeException("score must be a number")
        }
        val value =
            try {
                element.asBigDecimal.intValueExact()
            } catch (e: ArithmeticException) {
                throw GradeException("score must be a whole number from 0 to $MAX", e)
            }
        if (value !in 0..MAX) {
            throw GradeException("score must be a whole number from 0 to $MAX")
        }
        return value
    }

    private fun obj(element: JsonElement, what: String): JsonObject {
        if (!element.isJsonObject) {
            throw GradeException("$what must be an object")
        }
        return element.asJsonObject
    }
}
