package dx.docs.dokka

import org.jetbrains.dokka.model.Contravariance
import org.jetbrains.dokka.model.Covariance
import org.jetbrains.dokka.model.DefinitelyNonNullable
import org.jetbrains.dokka.model.Dynamic
import org.jetbrains.dokka.model.FunctionalTypeConstructor
import org.jetbrains.dokka.model.GenericTypeConstructor
import org.jetbrains.dokka.model.Invariance
import org.jetbrains.dokka.model.JavaObject
import org.jetbrains.dokka.model.Nullable
import org.jetbrains.dokka.model.PrimitiveJavaType
import org.jetbrains.dokka.model.Projection
import org.jetbrains.dokka.model.Star
import org.jetbrains.dokka.model.TypeAliased
import org.jetbrains.dokka.model.TypeParameter
import org.jetbrains.dokka.model.UnresolvedBound
import org.jetbrains.dokka.model.Void

/** Prints Dokka types as Kotlin source text and finds the ones Dokka could not resolve. */
internal object Types {
    /** Prints a type with simple names. */
    fun text(projection: Projection): String =
        when (projection) {
            is GenericTypeConstructor -> named(projection.dri.classNames, projection.projections)
            is FunctionalTypeConstructor -> function(projection)
            is Nullable -> text(projection.inner) + "?"
            is DefinitelyNonNullable -> text(projection.inner) + " & Any"
            is TypeAliased -> text(projection.typeAlias)
            is TypeParameter -> projection.name
            is Covariance<*> -> "out " + text(projection.inner)
            is Contravariance<*> -> "in " + text(projection.inner)
            is Invariance<*> -> text(projection.inner)
            is Star -> "*"
            is Void -> "Unit"
            is Dynamic -> "dynamic"
            is JavaObject -> "Any"
            is PrimitiveJavaType -> projection.name
            is UnresolvedBound -> projection.name
        }

    /** Returns the name of the first unresolved type inside [projection], or null. */
    fun unresolved(projection: Projection?): String? =
        when (projection) {
            null -> null
            is UnresolvedBound -> projection.name
            is GenericTypeConstructor -> firstUnresolved(projection.projections)
            is FunctionalTypeConstructor -> firstUnresolved(projection.projections)
            is Nullable -> unresolved(projection.inner)
            is DefinitelyNonNullable -> unresolved(projection.inner)
            is TypeAliased -> unresolved(projection.typeAlias) ?: unresolved(projection.inner)
            is Covariance<*> -> unresolved(projection.inner)
            is Contravariance<*> -> unresolved(projection.inner)
            is Invariance<*> -> unresolved(projection.inner)
            else -> null
        }

    private fun firstUnresolved(list: List<Projection>): String? = list.firstNotNullOfOrNull {
        unresolved(it)
    }

    private fun named(classNames: String?, arguments: List<Projection>): String {
        val simple = classNames.orEmpty().substringAfterLast('.')
        if (arguments.isEmpty()) {
            return simple
        }
        return simple + arguments.joinToString(", ", "<", ">") { text(it) }
    }

    private fun function(type: FunctionalTypeConstructor): String {
        val all = type.projections
        if (all.isEmpty()) {
            return "() -> Unit"
        }
        val parameters = all.dropLast(1).joinToString(", ", "(", ")") { text(it) }
        return parameters + " -> " + text(all.last())
    }
}
