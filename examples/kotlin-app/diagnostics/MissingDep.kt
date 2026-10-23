package diagnostics

import com.google.gson.JsonParser

object MissingDep {
    fun size(json: String): Int = JsonParser.parseString(json).asJsonArray.size()
}
