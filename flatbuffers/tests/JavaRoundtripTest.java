import com.google.flatbuffers.FlatBufferBuilder;
import dx.api.Shape;
import dx.base.Point;
import java.nio.ByteBuffer;

public final class JavaRoundtripTest {
  public static void main(String[] args) {
    FlatBufferBuilder builder = new FlatBufferBuilder(64);
    int name = builder.createString("circle");
    Shape.startShape(builder);
    Shape.addName(builder, name);
    Shape.addOrigin(builder, Point.createPoint(builder, 1.5F, -2.0F));
    builder.finish(Shape.endShape(builder));

    Shape shape = Shape.getRootAsShape(ByteBuffer.wrap(builder.sizedByteArray()));
    if (!"circle".equals(shape.name())
        || shape.origin().x() != 1.5F
        || shape.origin().y() != -2.0F) {
      throw new AssertionError("round trip mismatch");
    }
  }
}
