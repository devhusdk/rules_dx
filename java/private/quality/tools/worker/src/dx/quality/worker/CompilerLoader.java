package dx.quality.worker;

import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Hands out the class loader over a toolchain's compiler jars and keeps the last one.
 *
 * <p>Loading and JIT-warming kotlinc or scalac is most of a small check's cost, so a persistent
 * worker reuses the loader while requests name the same jars. The key is each jar's path, size and
 * modification time (following symlinks, as sandbox inputs are links), so a toolchain that changes
 * under the same path gets a fresh loader. Only one loader is kept: a build normally uses one
 * toolchain per language, and holding several compilers would multiply the worker's heap.
 */
public final class CompilerLoader {
  private static List<String> key;
  private static URLClassLoader loader;

  private CompilerLoader() {}

  /** The loader for {@code jars}; callers must not close it. */
  public static synchronized ClassLoader get(List<String> jars) throws IOException {
    List<String> wanted = new ArrayList<>();
    URL[] urls = new URL[jars.size()];
    for (int i = 0; i < urls.length; i++) {
      Path jar = Path.of(jars.get(i));
      wanted.add(jar + "\0" + Files.size(jar) + "\0" + Files.getLastModifiedTime(jar).toMillis());
      urls[i] = jar.toUri().toURL();
    }
    if (loader != null && wanted.equals(key)) {
      return loader;
    }
    URLClassLoader old = loader;
    loader = null;
    if (old != null) {
      old.close();
    }
    key = wanted;
    loader = new URLClassLoader(urls, ClassLoader.getPlatformClassLoader());
    return loader;
  }
}
