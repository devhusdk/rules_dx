package dx.docs.dokka

import org.jetbrains.dokka.CoreExtensions
import org.jetbrains.dokka.base.DokkaBase
import org.jetbrains.dokka.plugability.DokkaPlugin
import org.jetbrains.dokka.plugability.DokkaPluginApiPreview
import org.jetbrains.dokka.plugability.PluginApiPreviewAcknowledgement

/** Replaces Dokka's single-module generation with one that writes the JVM API record. */
class DxIrPlugin : DokkaPlugin() {
    /** The generation that stops after the documentable model and writes JSON. */
    @Suppress("unused")
    val dxGeneration by extending {
        CoreExtensions.generation providing
            ::DxIrGeneration override
            plugin<DokkaBase>().singleGeneration
    }

    @OptIn(DokkaPluginApiPreview::class)
    override fun pluginApiPreviewAcknowledgement(): PluginApiPreviewAcknowledgement =
        PluginApiPreviewAcknowledgement
}
