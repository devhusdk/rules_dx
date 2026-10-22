package grades

import com.google.common.base.Preconditions.checkArgument

/** One student and their scores. */
data class Student(val name: String, val scores: List<Int>) {
    init {
        checkArgument(name.isNotBlank(), "name is blank")
        checkArgument(scores.isNotEmpty(), "scores must not be empty")
    }

    /** Returns the average in tenths of a point, rounded half up. */
    fun averageTenths(): Int {
        val n = scores.size
        return ((scores.sum() * 20 + n) / (2 * n))
    }
}
