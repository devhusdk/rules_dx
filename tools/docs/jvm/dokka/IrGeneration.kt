package dx.docs.dokka

import dx.docs.jvm.ApiRecordWriter
import org.jetbrains.dokka.Timer
import org.jetbrains.dokka.base.DokkaBase
import org.jetbrains.dokka.base.generation.SingleModuleGeneration
import org.jetbrains.dokka.generation.Generation
import org.jetbrains.dokka.plugability.DokkaContext

/** What one in-process run extracts and where it reports. */
internal object Job {
    @Volatile var files: SourceMap? = null

    @Volatile var symbols: List<ApiRecordWriter.Symbol>? = null

    @Volatile var problems: List<String> = emptyList()
}

/** Runs Dokka's own model steps, then reads the public API from the merged module. */
class DxIrGeneration(private val context: DokkaContext) : Generation {
    override val generationName: String = "dx api records"

    override fun Timer.generate() {
        val files = checkNotNull(Job.files) { "the run has no source map" }
        val base = SingleModuleGeneration(context)
        report("Creating documentation models")
        val models = base.createDocumentationModels()
        report("Transforming documentation models")
        val transformed = base.transformDocumentationModelBeforeMerge(models)
        val merged = base.mergeDocumentationModels(transformed)
        val module =
            base.transformDocumentationModelAfterMerge(
                checkNotNull(merged) { "no public declarations to document" }
            )
        val signatures =
            context.single(
                checkNotNull(context.plugin(DokkaBase::class)) { "DokkaBase is not loaded" }
                    .signatureProvider
            )
        report("Reading the public API")
        val extractor = Extractor(signatures, files)
        Job.symbols = extractor.extract(module)
        Job.problems = extractor.problems
    }
}
