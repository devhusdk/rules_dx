package dx.docs.dokka

import dx.docs.jvm.ApiRecordWriter
import org.jetbrains.dokka.base.signatures.SignatureProvider
import org.jetbrains.dokka.links.DRI
import org.jetbrains.dokka.model.DAnnotation
import org.jetbrains.dokka.model.DClasslike
import org.jetbrains.dokka.model.DEnum
import org.jetbrains.dokka.model.DEnumEntry
import org.jetbrains.dokka.model.DFunction
import org.jetbrains.dokka.model.DInterface
import org.jetbrains.dokka.model.DModule
import org.jetbrains.dokka.model.DObject
import org.jetbrains.dokka.model.DPackage
import org.jetbrains.dokka.model.DProperty
import org.jetbrains.dokka.model.DTypeAlias
import org.jetbrains.dokka.model.Documentable
import org.jetbrains.dokka.model.DocumentableSource
import org.jetbrains.dokka.model.WithCompanion
import org.jetbrains.dokka.model.WithConstructors
import org.jetbrains.dokka.model.WithGenerics
import org.jetbrains.dokka.model.WithScope
import org.jetbrains.dokka.model.WithSupertypes
import org.jetbrains.dokka.model.WithTypealiases
import org.jetbrains.dokka.model.WithVisibility
import org.jetbrains.dokka.pages.ContentBreakLine
import org.jetbrains.dokka.pages.ContentNode
import org.jetbrains.dokka.pages.ContentText

/** Reads the public API out of Dokka's merged module into the shared API record symbols. */
internal class Extractor(
    private val signatures: SignatureProvider,
    private val files: SourceMap,
) {
    private val symbols = ArrayList<ApiRecordWriter.Symbol>()
    private val visited = HashSet<DRI>()

    /** Reasons the run must fail; the record is not written when any exist. */
    val problems = ArrayList<String>()

    /** Returns every public symbol of the module in a stable order. */
    fun extract(module: DModule): List<ApiRecordWriter.Symbol> {
        for (pack in module.packages) {
            packageOf(pack)
        }
        return symbols
    }

    private fun packageOf(pack: DPackage) {
        val name = pack.packageName
        if (name.isEmpty()) {
            return
        }
        val before = symbols.size
        members(pack, name, name)
        (pack as WithTypealiases).typealiases.forEach { alias(it, name) }
        if (symbols.size == before) {
            return
        }
        val file = symbols.subList(before, symbols.size).minOf { it.file }
        symbols.add(
            ApiRecordWriter.Symbol(
                name,
                null,
                "package",
                "package $name",
                Docs.render(one(pack.documentation)).body,
                null,
                null,
                file,
                1,
                null,
                null,
            )
        )
    }

    private fun members(
        scope: WithScope,
        owner: String,
        container: String,
    ) {
        val enum = scope is DEnum
        scope.functions
            .filterNot { enum && it.name in ENUM_FUNCTIONS }
            .forEach { function(it, container, false, null) }
        scope.properties
            .filterNot { enum && it.name == "entries" }
            .forEach { property(it, container) }
        scope.classlikes.forEach { classlike(it, owner) }
    }

    private fun classlike(
        item: DClasslike,
        container: String,
    ) {
        if (!isPublic(item) || !visited.add(item.dri)) {
            return
        }
        val qualified = Keys.classQualified(item.dri)
        val kind =
            when (item) {
                is DInterface -> "interface"
                is DObject -> "object"
                is DEnum -> "enum"
                is DAnnotation -> "annotation"
                else -> "class"
            }
        val located = locate(item, qualified) ?: return
        val doc = Docs.render(one(item.documentation))
        val supers = ArrayList<String>()
        if (item is WithSupertypes) {
            for (type in one(item.supertypes).orEmpty()) {
                checkType(Types.unresolved(type.typeConstructor), qualified)
                val name = Keys.classQualified(type.typeConstructor.dri)
                if (name !in IGNORED_SUPERTYPES) {
                    supers.add(name)
                }
            }
        }
        if (item is WithGenerics) {
            item.generics.forEach { generic ->
                generic.bounds.forEach { checkType(Types.unresolved(it), qualified) }
            }
        }
        symbols.add(
            ApiRecordWriter.Symbol(
                qualified,
                null,
                kind,
                signature(item),
                doc.body,
                null,
                null,
                located.first,
                located.second,
                container.ifEmpty { null },
                supers,
            )
        )
        if (item is WithConstructors) {
            item.constructors.forEach { function(it, qualified, true, doc) }
        }
        (item as? WithCompanion)?.companion?.let { classlike(it, qualified) }
        if (item is DEnum) {
            item.entries.forEach { entry(it, qualified, located) }
        }
        members(item, qualified, qualified)
        (item as? WithTypealiases)?.typealiases?.forEach { alias(it, qualified) }
    }

    private fun entry(
        item: DEnumEntry,
        owner: String,
        enumAt: Pair<String, Int>,
    ) {
        val qualified = "$owner.${item.name}"
        val line = files.lineMentioning(enumAt.first, enumAt.second, item.name)
        symbols.add(
            ApiRecordWriter.Symbol(
                qualified,
                null,
                "enum_constant",
                item.name,
                Docs.render(one(item.documentation)).body,
                null,
                null,
                enumAt.first,
                line,
                owner,
                null,
            )
        )
    }

    private fun alias(
        item: DTypeAlias,
        container: String,
    ) {
        if (!isPublic(item)) {
            return
        }
        val qualified = Keys.qualified(item.dri)
        val located = locate(item, qualified) ?: return
        one(item.underlyingType)?.let { checkType(Types.unresolved(it), qualified) }
        symbols.add(
            ApiRecordWriter.Symbol(
                qualified,
                null,
                "typealias",
                signature(item),
                Docs.render(one(item.documentation)).body,
                null,
                null,
                located.first,
                located.second,
                container,
                null,
            )
        )
    }

    private fun function(
        item: DFunction,
        container: String,
        constructor: Boolean,
        owner: Doc?,
    ) {
        if (!isPublic(item) || item.sources.isEmpty()) {
            return
        }
        val callable = item.dri.callable ?: return
        val qualified = Keys.qualified(item.dri)
        val located = locate(item, qualified) ?: return
        val doc = Docs.render(one(item.documentation))
        checkType(Types.unresolved(item.type), qualified)
        item.receiver?.let { checkType(Types.unresolved(it.type), qualified) }
        item.generics.forEach { generic ->
            generic.bounds.forEach { checkType(Types.unresolved(it), qualified) }
        }
        val params = ArrayList<ApiRecordWriter.Param>()
        for (parameter in item.parameters) {
            checkType(Types.unresolved(parameter.type), qualified)
            val name = parameter.name ?: "_"
            val text = doc.params[name] ?: owner?.params?.get(name) ?: ""
            params.add(ApiRecordWriter.Param(name, Types.text(parameter.type), text))
        }
        val type = Types.text(item.type)
        val returns =
            if (constructor || type == "Unit") {
                null
            } else {
                ApiRecordWriter.Returns(type, doc.returns.orEmpty())
            }
        symbols.add(
            ApiRecordWriter.Symbol(
                qualified,
                Keys.overload(callable),
                if (constructor) "constructor" else "function",
                signature(item),
                doc.body,
                params,
                returns,
                located.first,
                located.second,
                container.ifEmpty { null },
                null,
            )
        )
    }

    private fun property(
        item: DProperty,
        container: String,
    ) {
        if (!isPublic(item) || item.sources.isEmpty()) {
            return
        }
        val callable = item.dri.callable ?: return
        val qualified = Keys.qualified(item.dri)
        val located = locate(item, qualified) ?: return
        checkType(Types.unresolved(item.type), qualified)
        item.receiver?.let { checkType(Types.unresolved(it.type), qualified) }
        val receiver = callable.receiver?.let { Keys.erased(it) }
        symbols.add(
            ApiRecordWriter.Symbol(
                qualified,
                null,
                "property",
                signature(item),
                Docs.render(one(item.documentation)).body,
                null,
                null,
                located.first,
                located.second,
                container.ifEmpty { null },
                null,
                receiver,
            )
        )
    }

    private fun checkType(
        name: String?,
        where: String,
    ) {
        if (name != null) {
            problems.add("$where uses the unresolved type '$name'")
        }
    }

    private fun isPublic(item: Documentable): Boolean {
        if (item !is WithVisibility) {
            return true
        }
        val all = item.visibility.values
        return all.isNotEmpty() && all.all { it.name == "public" }
    }

    private fun locate(
        item: Documentable,
        qualified: String,
    ): Pair<String, Int>? {
        val sources = (item as? org.jetbrains.dokka.model.WithSources)?.sources
        val source: DocumentableSource = sources?.let { one(it) } ?: return null
        val exec = files.lookup(source.path)
        if (exec == null) {
            problems.add("$qualified is declared in ${source.path}, which is not one of the srcs")
            return null
        }
        val line = source.computeLineNumber()
        if (line == null || line < 1) {
            problems.add("$qualified has no source line")
            return null
        }
        return exec to line
    }

    private fun signature(item: Documentable): String = flatten(signatures.signature(item)).trim()

    private fun flatten(nodes: List<ContentNode>): String {
        val out = StringBuilder()
        for (node in nodes) {
            when (node) {
                is ContentText -> out.append(node.text)
                is ContentBreakLine -> out.append('\n')
                else -> out.append(flatten(node.children))
            }
        }
        return out.toString().replace(' ', ' ')
    }

    private fun <V : Any> one(map: Map<*, V>): V? = map.values.firstOrNull()

    private companion object {
        val ENUM_FUNCTIONS = setOf("values", "valueOf")
        val IGNORED_SUPERTYPES =
            setOf(
                "kotlin.Any",
                "kotlin.Enum",
                "kotlin.Annotation",
                "java.lang.Object",
                "java.lang.Enum",
            )
    }
}
