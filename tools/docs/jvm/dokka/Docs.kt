package dx.docs.dokka

import org.jetbrains.dokka.model.doc.A
import org.jetbrains.dokka.model.doc.B
import org.jetbrains.dokka.model.doc.BlockQuote
import org.jetbrains.dokka.model.doc.Br
import org.jetbrains.dokka.model.doc.CodeBlock
import org.jetbrains.dokka.model.doc.CodeInline
import org.jetbrains.dokka.model.doc.Deprecated
import org.jetbrains.dokka.model.doc.Description
import org.jetbrains.dokka.model.doc.DocTag
import org.jetbrains.dokka.model.doc.DocumentationLink
import org.jetbrains.dokka.model.doc.DocumentationNode
import org.jetbrains.dokka.model.doc.Em
import org.jetbrains.dokka.model.doc.H1
import org.jetbrains.dokka.model.doc.H2
import org.jetbrains.dokka.model.doc.H3
import org.jetbrains.dokka.model.doc.H4
import org.jetbrains.dokka.model.doc.H5
import org.jetbrains.dokka.model.doc.H6
import org.jetbrains.dokka.model.doc.HorizontalRule
import org.jetbrains.dokka.model.doc.I
import org.jetbrains.dokka.model.doc.Li
import org.jetbrains.dokka.model.doc.Ol
import org.jetbrains.dokka.model.doc.P
import org.jetbrains.dokka.model.doc.Param
import org.jetbrains.dokka.model.doc.Return
import org.jetbrains.dokka.model.doc.See
import org.jetbrains.dokka.model.doc.Since
import org.jetbrains.dokka.model.doc.Strong
import org.jetbrains.dokka.model.doc.Text
import org.jetbrains.dokka.model.doc.Throws
import org.jetbrains.dokka.model.doc.Ul

/** The parts of one KDoc comment, as Markdown. */
internal class Doc(
    val body: String,
    val params: Map<String, String>,
    val returns: String?,
) {
    companion object {
        val EMPTY = Doc("", emptyMap(), null)
    }
}

/** Converts Dokka's doc tags to the Markdown the shared site renders. */
internal object Docs {
    private val MAX_BLANKS = Regex("\n{3,}")

    /** Renders a documentation node; links become `jvm:` targets for the converter. */
    fun render(node: DocumentationNode?): Doc {
        if (node == null) {
            return Doc.EMPTY
        }
        val body = StringBuilder()
        val params = LinkedHashMap<String, String>()
        var returns: String? = null
        var deprecated: String? = null
        var since: String? = null
        val thrown = ArrayList<String>()
        val seen = ArrayList<String>()
        for (tag in node.children) {
            val text = blocks(tag.root.children).trim()
            when (tag) {
                is Description -> body.append(text)
                is Param -> params[tag.name] = text
                is Return -> returns = text
                is Deprecated -> deprecated = text
                is Since -> since = text
                is Throws -> thrown.add("`${tag.name}`" + if (text.isEmpty()) "" else ": $text")
                is See -> seen.add("`${tag.name}`" + if (text.isEmpty()) "" else ": $text")
                else -> Unit
            }
        }
        deprecated?.let { body.append("\n\n**Deprecated.** ").append(it) }
        if (thrown.isNotEmpty()) {
            body.append("\n\n**Throws**\n")
            thrown.forEach { body.append("\n- ").append(it) }
        }
        if (seen.isNotEmpty()) {
            body.append("\n\n**See also**\n")
            seen.forEach { body.append("\n- ").append(it) }
        }
        since?.let { body.append("\n\n**Since:** ").append(it) }
        return Doc(tidy(body.toString()), params, returns)
    }

    private fun tidy(text: String): String = MAX_BLANKS.replace(text.trim(), "\n\n")

    private fun isBlock(tag: DocTag): Boolean =
        tag is P ||
            tag is Ul ||
            tag is Ol ||
            tag is CodeBlock ||
            tag is BlockQuote ||
            tag is HorizontalRule ||
            tag is H1 ||
            tag is H2 ||
            tag is H3 ||
            tag is H4 ||
            tag is H5 ||
            tag is H6

    /** Joins block children with blank lines and runs of inline children together. */
    private fun blocks(children: List<DocTag>): String {
        val parts = ArrayList<String>()
        val inline = StringBuilder()
        for (child in children) {
            if (isBlock(child) || (child is org.jetbrains.dokka.model.doc.CustomDocTag)) {
                if (inline.isNotBlank()) {
                    parts.add(inline.toString().trim())
                }
                inline.clear()
                val text = block(child)
                if (text.isNotBlank()) {
                    parts.add(text)
                }
            } else {
                inline.append(inline(child))
            }
        }
        if (inline.isNotBlank()) {
            parts.add(inline.toString().trim())
        }
        return parts.joinToString("\n\n")
    }

    private fun block(tag: DocTag): String =
        when (tag) {
            is Ul -> tag.children.joinToString("\n") { "- " + item(it) }

            is Ol ->
                tag.children
                    .mapIndexed { index, child -> "${index + 1}. " + item(child) }
                    .joinToString("\n")

            is CodeBlock -> "```\n" + raw(tag) + "\n```"

            is BlockQuote -> blocks(tag.children).lines().joinToString("\n") { "> $it" }

            is HorizontalRule -> "---"

            is H1 -> "# " + inlines(tag)

            is H2 -> "## " + inlines(tag)

            is H3 -> "### " + inlines(tag)

            is H4 -> "#### " + inlines(tag)

            is H5 -> "##### " + inlines(tag)

            is H6 -> "###### " + inlines(tag)

            is P -> inlines(tag).trim()

            else -> blocks(tag.children)
        }

    private fun item(tag: DocTag): String {
        val text = if (tag is Li) blocks(tag.children) else blocks(listOf(tag))
        return text
            .lines()
            .mapIndexed { index, line -> if (index == 0) line else "  $line" }
            .joinToString("\n")
    }

    private fun inlines(tag: DocTag): String = tag.children.joinToString("") { inline(it) }

    private fun inline(tag: DocTag): String =
        when (tag) {
            is Text -> escape(tag.body)

            is B,
            is Strong -> "**" + inlines(tag) + "**"

            is I,
            is Em -> "*" + inlines(tag) + "*"

            is CodeInline -> "`" + raw(tag).replace("`", "") + "`"

            is Br -> "  \n"

            is DocumentationLink -> link(tag)

            is A -> {
                val href = tag.params["href"]
                if (href == null) inlines(tag) else "[" + inlines(tag).replace("]", "") + "]($href)"
            }

            else -> if (isBlock(tag)) block(tag) else inlines(tag)
        }

    private fun link(tag: DocumentationLink): String {
        val label = inlines(tag).replace("[", "").replace("]", "")
        val target = Keys.linkTarget(tag.dri) ?: return "`" + raw(tag).replace("`", "") + "`"
        return "[$label](jvm:$target)"
    }

    private fun raw(tag: DocTag): String =
        if (tag is Text) tag.body else tag.children.joinToString("") { raw(it) }

    private fun escape(text: String): String {
        val out = StringBuilder(text.length + 8)
        for (ch in text) {
            when (ch) {
                '<' -> out.append("&lt;")

                '>' -> out.append("&gt;")

                '&' -> out.append("&amp;")

                '\\',
                '`',
                '*',
                '_',
                '[',
                ']' -> out.append('\\').append(ch)

                else -> out.append(ch)
            }
        }
        return out.toString()
    }
}
