package dx.quality.javac;

import dx.quality.worker.Worker;

/**
 * Entry point: {@code Main @params}, or a Bazel persistent worker with {@code --persistent_worker}.
 *
 * <p>Exits 0 whenever javac ran, whatever javac returned, because the log carries javac's own
 * exit status. A non-zero exit means the driver itself failed.
 */
public final class Main {
  private Main() {}

  public static void main(String[] args) {
    Worker.main("javac_check", args, JavacCheck::check);
  }
}
