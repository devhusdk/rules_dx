package dx.docs.dokka

import org.jetbrains.dokka.links.Callable
import org.jetbrains.dokka.links.DRI
import org.jetbrains.dokka.links.JavaClassReference
import org.jetbrains.dokka.links.Nullable
import org.jetbrains.dokka.links.PointingToDeclaration
import org.jetbrains.dokka.links.TypeConstructor
import org.jetbrains.dokka.links.TypeParam
import org.jetbrains.dokka.links.TypeReference
import org.jetbrains.dokka.links.Vararg

/** Builds the symbol keys that `jvm_convert` reads, from Dokka's DRIs. */
internal object Keys {
    private const val CONSTRUCTOR = "<init>"
    private const val ANY = "Any"

    /** The erased simple name of a type, with a vararg written `T[]` and no `?`. */
    fun erased(reference: TypeReference?): String =
        when (reference) {
            is TypeConstructor -> reference.fullyQualifiedName.substringAfterLast('.')
            is Nullable -> erased(reference.wrapped)
            is TypeParam -> reference.bounds.firstOrNull()?.let { erased(it) } ?: ANY
            is Vararg -> erased(reference.elementType) + "[]"
            is JavaClassReference -> reference.name.substringAfterLast('.')
            else -> ANY
        }

    /** The overload list of a callable: an extension's receiver comes first. */
    fun overload(callable: Callable): List<String> =
        listOfNotNull(callable.receiver).map { erased(it) } + callable.params.map { erased(it) }

    /** The dotted name of the class a DRI points into, or the package when it names no class. */
    fun classQualified(dri: DRI): String = join(dri.packageName.orEmpty(), dri.classNames)

    /** The dotted name of a DRI, ending in the callable name when it has one. */
    fun qualified(dri: DRI): String {
        val callable = dri.callable ?: return classQualified(dri)
        val name =
            if (callable.name == CONSTRUCTOR) {
                dri.classNames.orEmpty().substringAfterLast('.')
            } else {
                callable.name
            }
        return join(classQualified(dri), name)
    }

    /** The key of a documentation link target, or null when it is not a declaration. */
    fun linkTarget(dri: DRI): String? {
        if (dri.target !is PointingToDeclaration) {
            return null
        }
        val callable = dri.callable
        if (callable == null) {
            return classQualified(dri).ifEmpty { null }
        }
        val base = qualified(dri)
        if (callable.isProperty) {
            return callable.receiver?.let { "$base@${erased(it)}" } ?: base
        }
        return base + "(" + overload(callable).joinToString(",") + ")"
    }

    private fun join(left: String, right: String?): String =
        when {
            right.isNullOrEmpty() -> left
            left.isEmpty() -> right
            else -> "$left.$right"
        }
}
