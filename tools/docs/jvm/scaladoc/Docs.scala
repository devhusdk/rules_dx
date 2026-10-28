package dx.docs.scaladoc

import scala.tools.nsc.doc.base.comment._
import scala.tools.nsc.doc.base.{
  LinkTo,
  LinkToExternalTpl,
  LinkToMember,
  LinkToTpl
}

/** Renders Scaladoc comments as Markdown, with resolved links written as `jvm:`
  * links.
  */
final class Docs(targetKey: Any => Option[String]) {

  /** The Markdown of one comment: its body, then the thrown exceptions. */
  def render(comment: Option[Comment]): String =
    comment match {
      case None    => ""
      case Some(c) =>
        val throwsDocs = c.throws.toList.sortBy(_._1).map { case (name, text) =>
          s"- `$name`: ${body(text).replace("\n\n", " ")}"
        }
        (List(
          body(c.body)
        ) ++ (if (throwsDocs.isEmpty) Nil
              else List("**Throws**\n\n" + throwsDocs.mkString("\n"))))
          .filter(_.nonEmpty)
          .mkString("\n\n")
    }

  def body(value: Body): String =
    value.blocks.map(block).filter(_.nonEmpty).mkString("\n\n")

  def block(value: Block): String =
    value match {
      case Title(text, level)   => "#" * math.max(1, level) + " " + inline(text)
      case Paragraph(text)      => inline(text).trim
      case Code(data)           => "```\n" + data.stripLineEnd + "\n```"
      case UnorderedList(items) =>
        items.map(item => "- " + indent(block(item))).mkString("\n")
      case OrderedList(items, _) =>
        items.zipWithIndex
          .map { case (item, index) => s"${index + 1}. " + indent(block(item)) }
          .mkString("\n")
      case DefinitionList(items) =>
        items.toList
          .map { case (term, definition) =>
            s"- ${inline(term)}: ${indent(block(definition))}"
          }
          .mkString("\n")
      case HorizontalRule() => "---"
      case _                => ""
    }

  private def indent(text: String): String = text.replace("\n", "\n  ")

  def inline(value: Inline): String =
    value match {
      case Chain(items)            => items.map(inline).mkString
      case Italic(text)            => "*" + inline(text) + "*"
      case Bold(text)              => "**" + inline(text) + "**"
      case Underline(text)         => inline(text)
      case Superscript(text)       => inline(text)
      case Subscript(text)         => inline(text)
      case Monospace(text)         => "`" + inline(text).replace("`", "") + "`"
      case Text(text)              => text
      case Summary(text)           => inline(text)
      case Link(target, title)     => "[" + inline(title) + "](" + target + ")"
      case EntityLink(title, link) => entity(inline(title), link)
      case _                       => ""
    }

  private def entity(label: String, link: LinkTo): String = {
    val target =
      link match {
        case l: LinkToTpl[_]       => targetKey(l.tpl)
        case l: LinkToMember[_, _] => targetKey(l.mbr)
        case l: LinkToExternalTpl  => targetKey(l.tpl)
        case _                     => None
      }
    target match {
      case Some(key) => "[" + label.replace("`", "") + "](jvm:" + key + ")"
      case None      => "`" + label.replace("`", "") + "`"
    }
  }
}
