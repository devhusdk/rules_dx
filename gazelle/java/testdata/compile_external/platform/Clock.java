package platform;

import java.time.Instant;
import java.util.function.Supplier;

public class Clock {
  public Supplier<Instant> now() {
    return Instant::now;
  }
}
