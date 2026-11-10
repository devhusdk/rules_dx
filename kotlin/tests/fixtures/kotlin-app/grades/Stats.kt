package grades

import com.google.common.collect.ImmutableListMultimap
import com.google.common.collect.Multimaps

/** Ranks and groups students. */
object Stats {
    /** Formats tenths as a decimal. */
    fun tenths(value: Int): String = "${value / 10}.${value % 10}"

    /** Returns the letter grade for an average in tenths. */
    fun letter(averageTenths: Int): Char =
        when {
            averageTenths >= 900 -> 'A'
            averageTenths >= 800 -> 'B'
            averageTenths >= 700 -> 'C'
            averageTenths >= 600 -> 'D'
            else -> 'F'
        }

    /** Returns students from best to worst average, ties by name. */
    fun ranked(students: List<Student>): List<Student> =
        students.sortedWith(compareByDescending<Student> { it.averageTenths() }.thenBy { it.name })

    /** Groups students by letter grade, best first. */
    fun byLetter(students: List<Student>): ImmutableListMultimap<Char, Student> =
        Multimaps.index(ranked(students)) { letter(it.averageTenths()) }
}
