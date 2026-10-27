package app;

import java.util.List;
import javax.annotation.Nonnull;
import util.Widget;

public class App {
  @Nonnull
  public List<Widget> widgets() {
    return List.of(new Widget());
  }
}
