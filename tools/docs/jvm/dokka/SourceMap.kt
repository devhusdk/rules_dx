package dx.docs.dokka

import java.io.File
import java.nio.charset.StandardCharsets

/** Maps the absolute paths Dokka reports back to the exact execution paths of the srcs manifest. */
internal class SourceMap(val execPaths: List<String>) {
    private val byPath = HashMap<String, String>()
    private val lines = HashMap<String, List<String>>()

    init {
        for (exec in execPaths) {
            val file = File(exec)
            byPath[file.absoluteFile.normalize().path] = exec
            byPath[file.canonicalPath] = exec
        }
    }

    /** The manifest path of a source Dokka reported, or null when it is not one of ours. */
    fun lookup(path: String): String? {
        val file = File(path)
        return byPath[file.absoluteFile.normalize().path] ?: byPath[file.canonicalPath]
    }

    /** The first line at or after [from] that mentions [word], or [from]. */
    fun lineMentioning(exec: String, from: Int, word: String): Int {
        val text = lines.getOrPut(exec) { File(exec).readLines(StandardCharsets.UTF_8) }
        val pattern = Regex("\\b" + Regex.escape(word) + "\\b")
        for (index in from until text.size) {
            if (pattern.containsMatchIn(text[index])) {
                return index + 1
            }
        }
        return from
    }
}
