package dx.bench;

import java.io.IOException;
import java.io.OutputStream;
import java.io.UncheckedIOException;
import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;
import java.util.stream.Stream;
import org.openjdk.jmh.generators.bytecode.JmhBytecodeGenerator;

/**
 * Runs the JMH bytecode generator over a compiled library.
 *
 * <p>Args: lib.jar out.srcjar out.resources.jar classpath-jar...
 */
public final class JmhGenerate {
  private static final long EPOCH = 315532800000L;

  private JmhGenerate() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 3) {
      System.err.println("usage: JmhGenerate lib.jar out.srcjar out.resources.jar classpath-jar...");
      System.exit(2);
    }
    Path work = Files.createTempDirectory("jmh-generate");
    Path classes = work.resolve("classes");
    Path sources = work.resolve("sources");
    Path resources = work.resolve("resources");
    Files.createDirectories(classes);
    Files.createDirectories(sources);
    Files.createDirectories(resources);
    unzip(Paths.get(args[0]), classes);

    List<URL> urls = new ArrayList<>();
    for (int i = 3; i < args.length; i++) {
      urls.add(url(Paths.get(args[i])));
    }
    ClassLoader parent = JmhGenerate.class.getClassLoader();
    Thread.currentThread().setContextClassLoader(new URLClassLoader(urls.toArray(new URL[0]), parent));

    JmhBytecodeGenerator.main(
        new String[] {
          classes.toString(), sources.toString(), resources.toString(), "reflection"
        });

    zip(sources, Paths.get(args[1]));
    zip(resources, Paths.get(args[2]));
  }

  private static URL url(Path path) throws MalformedURLException {
    return path.toAbsolutePath().toUri().toURL();
  }

  private static void unzip(Path jar, Path dir) throws IOException {
    try (ZipFile zip = new ZipFile(jar.toFile())) {
      Enumeration<? extends ZipEntry> entries = zip.entries();
      while (entries.hasMoreElements()) {
        ZipEntry entry = entries.nextElement();
        Path target = dir.resolve(entry.getName()).normalize();
        if (!target.startsWith(dir)) {
          throw new IOException("entry escapes the output directory: " + entry.getName());
        }
        if (entry.isDirectory()) {
          Files.createDirectories(target);
        } else {
          Files.createDirectories(target.getParent());
          Files.copy(zip.getInputStream(entry), target);
        }
      }
    }
  }

  private static void zip(Path dir, Path out) throws IOException {
    List<Path> files;
    try (Stream<Path> walk = Files.walk(dir)) {
      files = walk.filter(Files::isRegularFile).sorted().toList();
    }
    try (OutputStream raw = Files.newOutputStream(out);
        ZipOutputStream zip = new ZipOutputStream(raw)) {
      for (Path file : files) {
        ZipEntry entry = new ZipEntry(dir.relativize(file).toString().replace('\\', '/'));
        entry.setTime(EPOCH);
        zip.putNextEntry(entry);
        try {
          Files.copy(file, zip);
        } catch (IOException e) {
          throw new UncheckedIOException(e);
        }
        zip.closeEntry();
      }
    }
  }
}
