package dx.docs.scaladoc

import dx.docs.jvm.ApiRecordWriter
import java.io.File
import java.nio.charset.StandardCharsets
import java.nio.file.{Files, Path, Paths}
import scala.jdk.CollectionConverters._
import scala.tools.nsc.doc
import scala.tools.nsc.doc.DocFactory
import scala.tools.nsc.reporters.StoreReporter

/** Runs the Scala 2.13 Scaladoc model in-process and writes one API record
  * file.
  */
object Main {
  private val ToolName = "scaladoc"
  private val Flags =
    List("--srcs-manifest", "--classpath-file", "--release", "--out")

  def main(args: Array[String]): Unit = {
    val code =
      try run(args)
      catch {
        case error: Exception =>
          System.err.println(
            s"scaladoc_extract: ${Option(error.getMessage).getOrElse(error.toString)}"
          )
          1
      }
    sys.exit(code)
  }

  private def usage(message: String): Int = {
    System.err.println(s"scaladoc_extract: $message")
    System.err.println(
      s"usage: scaladoc_extract ${Flags.map(_ + " VALUE").mkString(" ")}"
    )
    2
  }

  private def lines(path: String): List[String] =
    Files
      .readAllLines(Paths.get(path), StandardCharsets.UTF_8)
      .asScala
      .toList
      .filter(_.nonEmpty)

  private def run(args: Array[String]): Int = {
    if (!scala.util.Properties.versionNumberString.startsWith("2.13.")) {
      System.err.println("scaladoc_extract: only Scala 2.13 is supported")
      return 1
    }
    if (args.length % 2 != 0) return usage("every flag needs a value")
    val flags = args.grouped(2).map(pair => pair(0) -> pair(1)).toMap
    val unknown = flags.keys.find(!Flags.contains(_))
    if (unknown.isDefined) return usage(s"unknown argument '${unknown.get}'")
    if (Flags.exists(!flags.contains(_)))
      return usage(s"${Flags.mkString(", ")} are required")

    val sources = lines(flags("--srcs-manifest")).map(_.split('\t')(0))
    val missingSource =
      sources.find(source => !Files.isRegularFile(Paths.get(source)))
    if (missingSource.isDefined) {
      System.err.println(
        s"scaladoc_extract: missing source ${missingSource.get}"
      )
      return 1
    }
    if (sources.isEmpty) {
      System.err.println("scaladoc_extract: the srcs manifest lists no sources")
      return 1
    }
    val classpath = lines(flags("--classpath-file"))
    val missingEntry = classpath.find(entry => !Files.exists(Paths.get(entry)))
    if (missingEntry.isDefined) {
      System.err.println(
        s"scaladoc_extract: missing classpath entry ${missingEntry.get}"
      )
      return 1
    }
    val out = Paths.get(flags("--out"))
    Files.deleteIfExists(out)
    val scratch = Files.createTempDirectory("dx-scaladoc")
    try
      generate(
        sources,
        classpath.map(entry => new File(entry).getAbsolutePath),
        out,
        scratch
      )
    finally deleteTree(scratch)
  }

  private def generate(
      sources: List[String],
      classpath: List[String],
      out: Path,
      scratch: Path
  ): Int = {
    val problems = scala.collection.mutable.ListBuffer.empty[String]
    val settings = new doc.Settings(
      message => { problems += message; () },
      message => { problems += message; () }
    )
    // The compiler cannot run without scala-library; fall back to the one this adapter runs on.
    val library = classOf[
      scala.Option[_]
    ].getProtectionDomain.getCodeSource.getLocation.toURI
    val withLibrary =
      if (
        classpath
          .exists(entry => new File(entry).getName.startsWith("scala-library"))
      ) classpath
      else classpath :+ new File(library).getAbsolutePath
    settings.classpath.value = withLibrary.mkString(File.pathSeparator)
    settings.outdir.value = scratch.toString
    // Every classpath jar is an "external" library, so links into dependencies resolve to
    // their templates; the URL is never used.
    settings.docExternalDoc.value = classpath
      .filter(_.endsWith(".jar"))
      .map(jar => s"$jar#https://dx.invalid/")
    val reporter = new StoreReporter(settings)
    val universe =
      new DocFactory(reporter, settings).makeUniverse(Left(sources))
    val errors = reporter.infos
      .filter(_.severity == reporter.ERROR)
      .map(info => info.pos.toString + ": " + info.msg)
      .toList
    if (errors.nonEmpty || problems.toList.nonEmpty) {
      (errors ++ problems.toList).foreach(error =>
        System.err.println(s"scaladoc_extract: $error")
      )
      return 1
    }
    if (universe.isEmpty) {
      System.err.println(
        "scaladoc_extract: the sources could not be documented"
      )
      return 1
    }
    val symbols = new Model(
      sources
        .map(source =>
          Paths.get(source).toAbsolutePath.normalize.toString -> source
        )
        .toMap
    ).symbols(universe.get)
    if (symbols.isEmpty) {
      System.err.println(
        "scaladoc_extract: no public declarations in the sources"
      )
      return 1
    }
    val unresolved = symbols.find(_.signature.contains("<error>"))
    if (unresolved.isDefined) {
      System.err.println(
        s"scaladoc_extract: unresolved type in ${unresolved.get.qualified}: ${unresolved.get.signature}"
      )
      return 1
    }
    ApiRecordWriter.write(
      out,
      "scala",
      ToolName,
      scala.util.Properties.versionNumberString,
      symbols.asJava
    )
    0
  }

  private def deleteTree(root: Path): Unit = {
    val stream = Files.walk(root)
    try
      stream.iterator().asScala.toList.reverse.foreach(Files.deleteIfExists(_))
    finally stream.close()
  }
}
