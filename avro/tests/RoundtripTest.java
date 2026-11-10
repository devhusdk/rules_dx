import dx.test.Base;
import dx.test.Ping;
import dx.test.Shape;
import java.io.ByteArrayOutputStream;
import org.apache.avro.io.BinaryDecoder;
import org.apache.avro.io.BinaryEncoder;
import org.apache.avro.io.DecoderFactory;
import org.apache.avro.io.EncoderFactory;
import org.apache.avro.specific.SpecificDatumReader;
import org.apache.avro.specific.SpecificDatumWriter;

public final class RoundtripTest {
  public static void main(String[] args) throws Exception {
    Shape shape = Shape.newBuilder().setName("circle").setBase(new Base(7L)).build();
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    BinaryEncoder encoder = EncoderFactory.get().binaryEncoder(out, null);
    new SpecificDatumWriter<Shape>(Shape.class).write(shape, encoder);
    encoder.flush();

    BinaryDecoder decoder = DecoderFactory.get().binaryDecoder(out.toByteArray(), null);
    Shape read = new SpecificDatumReader<Shape>(Shape.class).read(null, decoder);
    if (!"circle".equals(read.getName().toString()) || read.getBase().getId() != 7L) {
      throw new AssertionError("round trip mismatch");
    }
    if (!"hi".equals(Ping.newBuilder().setNote("hi").build().getNote().toString())) {
      throw new AssertionError("avdl record mismatch");
    }
  }
}
