package spotbugs_owned;

import spotbugs_owned.a.Shape;
import spotbugs_owned.b.Sized;

public class Collide implements Sized {
  @Override
  public int size() {
    return Shape.sides();
  }
}
