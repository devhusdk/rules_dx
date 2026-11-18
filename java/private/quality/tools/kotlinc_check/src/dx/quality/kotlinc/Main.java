package dx.quality.kotlinc;

import dx.quality.worker.Worker;

/**
 * Entry point: {@code Main @params}, or a Bazel persistent worker with {@code --persistent_worker}.
 *
 * <p>Exits 0 whenever kotlinc ran, whatever kotlinc returned, because the log carries kotlinc's own
 * exit status. A non-zero exit means the driver itself failed.
 */
public final class Main {
  private Main() {}

  public static void main(String[] args) {
    Worker.main("kotlinc_check", args, KotlincCheck::check);
  }
}
