import dx.api.KindSchema;
import dx.api.ShapeSchema;
import java.nio.ByteBuffer;
import org.capnproto.ArrayOutputStream;
import org.capnproto.MessageBuilder;
import org.capnproto.MessageReader;
import org.capnproto.Serialize;

public final class JavaRoundtripTest {
  public static void main(String[] args) throws Exception {
    MessageBuilder message = new MessageBuilder();
    ShapeSchema.Shape.Builder shape = message.initRoot(ShapeSchema.Shape.factory);
    shape.setName("circle");
    shape.initOrigin().setX(1.5F);
    shape.getOrigin().setY(-2.0F);
    shape.setKind(KindSchema.Kind.SQUARE);

    ByteBuffer bytes =
        ByteBuffer.allocate((int) Serialize.computeSerializedSizeInWords(message) * 8);
    Serialize.write(new ArrayOutputStream(bytes), message);
    bytes.rewind();

    MessageReader reader = Serialize.read(bytes);
    ShapeSchema.Shape.Reader read = reader.getRoot(ShapeSchema.Shape.factory);
    if (!"circle".equals(read.getName().toString())
        || read.getOrigin().getX() != 1.5F
        || read.getOrigin().getY() != -2.0F
        || read.getKind() != KindSchema.Kind.SQUARE) {
      throw new AssertionError("round trip mismatch");
    }
  }
}
