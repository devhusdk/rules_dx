package grades

import com.google.common.collect.ImmutableListMultimap
import org.junit.Assert.assertEquals
import org.junit.Test

class StatsTest {
    private val ada = Student("ada", listOf(95, 88, 92))
    private val bo = Student("bo", listOf(72, 65, 80))
    private val cy = Student("cy", listOf(100, 100, 90))

    @Test
    fun averagesRoundHalfUpInTenths() {
        assertEquals(917, ada.averageTenths())
        assertEquals(723, bo.averageTenths())
        assertEquals(967, cy.averageTenths())
        assertEquals(5, Student("x", listOf(0, 1)).averageTenths())
    }

    @Test
    fun formatsTenths() {
        assertEquals("91.7", Stats.tenths(917))
        assertEquals("100.0", Stats.tenths(1000))
        assertEquals("0.5", Stats.tenths(5))
    }

    @Test
    fun lettersFollowTheBoundaries() {
        assertEquals('A', Stats.letter(900))
        assertEquals('B', Stats.letter(899))
        assertEquals('C', Stats.letter(700))
        assertEquals('D', Stats.letter(600))
        assertEquals('F', Stats.letter(599))
    }

    @Test
    fun ranksBestFirstAndBreaksTiesByName() {
        val twin = Student("aa", listOf(100, 100, 90))
        assertEquals(
            listOf("aa", "cy", "ada", "bo"),
            Stats.ranked(listOf(bo, ada, cy, twin)).map { it.name },
        )
    }

    @Test
    fun groupsByLetter() {
        val grouped: ImmutableListMultimap<Char, Student> = Stats.byLetter(listOf(ada, bo, cy))
        assertEquals(listOf("cy", "ada"), grouped['A'].map { it.name })
        assertEquals(listOf("bo"), grouped['C'].map { it.name })
        assertEquals(setOf('A', 'C'), grouped.keySet())
    }
}
