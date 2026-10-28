package dx.docs.scaladoc

import dx.docs.jvm.ApiRecordWriter
import dx.docs.jvm.ApiRecordWriter.{Param, Returns, Symbol}
import java.nio.file.Paths
import scala.collection.mutable
import scala.language.existentials
import scala.jdk.CollectionConverters._
import scala.tools.nsc.doc.Universe
import scala.tools.nsc.doc.base.comment.Comment
import scala.tools.nsc.doc.model._

/** Walks the Scaladoc model and maps every public declaration to an API record
  * symbol.
  */
final class Model(files: Map[String, String]) {
  private val docs = new Docs(key)
  private val out = mutable.LinkedHashMap.empty[String, Symbol]

  def symbols(universe: Universe): List[Symbol] = {
    visit(universe.rootPackage)
    out.values.toList
  }

  private def visit(owner: DocTemplateEntity): Unit =
    owner.members.foreach { member =>
      if (shown(owner, member)) {
        member match {
          case pkg: Package           => visit(pkg)
          case tpl: DocTemplateEntity =>
            template(owner, tpl)
            visit(tpl)
          case c: Constructor  => constructor(owner, c)
          case d: Def          => method(owner, d)
          case v: Val          => value(owner, v)
          case a: AliasType    => alias(owner, a)
          case a: AbstractType => abstractType(owner, a)
          case _               =>
        }
      }
    }

  private def shown(owner: DocTemplateEntity, member: MemberEntity): Boolean = {
    val definedHere =
      member.inDefinitionTemplates.headOption.forall(t => same(t, owner))
    val useCase = member match {
      case n: NonTemplateMemberEntity => n.isUseCase
      case _                          => false
    }
    member.visibility.isPublic && definedHere && member.byConversion.isEmpty && !useCase && !synthetic(
      member
    )
  }

  private def same(a: TemplateEntity, b: TemplateEntity): Boolean =
    a.qualifiedName == b.qualifiedName && a.isObject == b.isObject

  private def sym(entity: Entity) =
    entity.asInstanceOf[ModelFactory#EntityImpl].sym

  private def synthetic(member: MemberEntity): Boolean = sym(member).isSynthetic

  // Names and keys

  private def typeName(tpl: TemplateEntity): String =
    tpl.qualifiedName + (if (tpl.isObject) "$" else "")

  private def erase(text: String): String = {
    var type0 = text.trim
    if (type0.startsWith("=> ")) type0 = type0.drop(3)
    val repeated = type0.endsWith("*")
    if (repeated) type0 = type0.dropRight(1)
    val erased =
      if (type0.contains("=>")) "Function"
      else if (type0.startsWith("(")) "Tuple"
      else {
        val bracket = type0.indexOf('[')
        val bare = if (bracket >= 0) type0.substring(0, bracket) else type0
        bare.substring(bare.lastIndexOf('.') + 1)
      }
    if (repeated) erased + "[]" else erased
  }

  private def overload(lists: List[List[ValueParam]]): List[String] =
    lists.flatten.map(p => erase(p.resultType.name))

  private def callKey(
      owner: String,
      name: String,
      lists: List[List[ValueParam]]
  ): String =
    owner + "." + name + "(" + overload(lists).mkString(",") + ")"

  private def key(target: Any): Option[String] =
    target match {
      case c: Constructor =>
        Some(callKey(typeName(c.inTemplate), c.inTemplate.name, c.valueParams))
      case d: Def =>
        Some(callKey(typeName(d.inTemplate), d.name, d.valueParams))
      case t: TemplateEntity => Some(typeName(t))
      case m: MemberEntity   => Some(typeName(m.inTemplate) + "." + m.name)
      case _                 => None
    }

  // Rendering

  private def tparams(entity: HigherKinded): String =
    if (entity.typeParams.isEmpty) ""
    else
      entity.typeParams
        .map { p =>
          p.variance + p.name + p.lo.map(" >: " + _.name).getOrElse("") + p.hi
            .map(" <: " + _.name)
            .getOrElse("")
        }
        .mkString("[", ", ", "]")

  private def paramLists(lists: List[List[ValueParam]]): String =
    lists.map { list =>
      val implicitly = if (list.exists(_.isImplicit)) "implicit " else ""
      list
        .map(p => p.name + ": " + p.resultType.name)
        .mkString("(", ", ", ")")
        .replace("(", "(" + implicitly)
    }.mkString

  private def params(
      lists: List[List[ValueParam]],
      comment: Option[Comment]
  ): java.util.List[Param] =
    lists.flatten.map { p =>
      new Param(
        p.name,
        p.resultType.name,
        comment.flatMap(_.valueParams.get(p.name)).map(docs.body).getOrElse("")
      )
    }.asJava

  private def where(entity: Entity): (String, Int) = {
    val position = entity match {
      case t: DocTemplateEntity if t.inSource.isDefined =>
        Some((t.inSource.get._1.path, t.inSource.get._2))
      case _ =>
        val p = sym(entity).pos
        if (p.isDefined) Some((p.source.file.path, p.line)) else None
    }
    val (path, line) = position.getOrElse(
      throw new IllegalStateException(
        s"no source position for ${entity.qualifiedName}"
      )
    )
    val resolved =
      files.getOrElse(Paths.get(path).toAbsolutePath.normalize.toString, path)
    (resolved, line)
  }

  private def add(
      owner: DocTemplateEntity,
      member: MemberEntity,
      qualified: String,
      over: Option[List[String]],
      kind: String,
      signature: String,
      parameters: java.util.List[Param] = null,
      returns: Option[Returns] = None,
      parents: List[String] = Nil
  ): Unit = {
    val (file, line) = where(member)
    val memberOf =
      if (owner.isRootPackage || owner.isPackage) null else typeName(owner)
    val symbol = new Symbol(
      qualified,
      over.map(_.asJava).orNull,
      kind,
      signature,
      docs.render(member.comment),
      parameters,
      returns.orNull,
      file,
      line,
      memberOf,
      parents.asJava
    )
    out.put(
      qualified + over.fold("")(_.mkString("(", ",", ")")) + "#" + kind,
      symbol
    )
  }

  private def template(
      owner: DocTemplateEntity,
      tpl: DocTemplateEntity
  ): Unit = {
    val (kind, word) =
      if (tpl.isObject) ("object", "object")
      else if (tpl.isTrait) ("trait", "trait")
      else if (sym(tpl).isCaseClass) ("class", "case class")
      else ("class", "class")
    val extra = tpl match {
      case h: HigherKinded => tparams(h)
      case _               => ""
    }
    val parents = tpl.parentTypes.map(_._1).filter(_ != null).map(typeName)
    add(
      owner,
      tpl,
      typeName(tpl),
      None,
      kind,
      s"$word ${tpl.name}$extra",
      parents = parents.distinct
    )
  }

  private def constructor(owner: DocTemplateEntity, c: Constructor): Unit = {
    val comment = c.comment.orElse(owner.comment)
    add(
      owner,
      c,
      typeName(owner) + "." + owner.name,
      Some(overload(c.valueParams)),
      "constructor",
      "new " + owner.name + paramLists(c.valueParams),
      parameters = params(c.valueParams, comment)
    )
  }

  private def method(owner: DocTemplateEntity, d: Def): Unit = {
    val returns = d.comment
      .flatMap(_.result)
      .map(r => new Returns(d.resultType.name, docs.body(r)))
    add(
      owner,
      d,
      typeName(owner) + "." + d.name,
      Some(overload(d.valueParams)),
      "function",
      "def " + d.name + tparams(d) + paramLists(
        d.valueParams
      ) + ": " + d.resultType.name,
      parameters = params(d.valueParams, d.comment),
      returns = returns
    )
  }

  private def value(owner: DocTemplateEntity, v: Val): Unit = {
    val word = if (v.isVar) "var" else if (v.isLazyVal) "lazy val" else "val"
    add(
      owner,
      v,
      typeName(owner) + "." + v.name,
      None,
      "property",
      s"$word ${v.name}: ${v.resultType.name}"
    )
  }

  private def alias(owner: DocTemplateEntity, a: AliasType): Unit =
    add(
      owner,
      a,
      typeName(owner) + "." + a.name,
      None,
      "typealias",
      "type " + a.name + tparams(a) + " = " + a.alias.name
    )

  private def abstractType(owner: DocTemplateEntity, a: AbstractType): Unit =
    add(
      owner,
      a,
      typeName(owner) + "." + a.name,
      None,
      "typealias",
      "type " + a.name + tparams(a) + a.lo
        .map(" >: " + _.name)
        .getOrElse("") + a.hi.map(" <: " + _.name).getOrElse("")
    )
}
