import dx.api.Shape;
import org.apache.thrift.TDeserializer;
import org.apache.thrift.TSerializer;
import org.apache.thrift.protocol.TBinaryProtocol;

public final class JavaRoundtripTest {
  public static void main(String[] args) throws Exception {
    Shape shape = new Shape();
    shape.setName("circle");
    shape.setOrigin(new dx.base.Point());
    shape.getOrigin().setX(1.5);
    shape.getOrigin().setY(-2.0);

    byte[] bytes = new TSerializer(new TBinaryProtocol.Factory()).serialize(shape);
    Shape read = new Shape();
    new TDeserializer(new TBinaryProtocol.Factory()).deserialize(read, bytes);
    if (!"circle".equals(read.getName())
        || read.getOrigin().getX() != 1.5
        || read.getOrigin().getY() != -2.0) {
      throw new AssertionError("round trip mismatch");
    }
  }
}
