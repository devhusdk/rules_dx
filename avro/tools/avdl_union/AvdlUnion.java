package dx.avro.tools;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import org.apache.avro.Schema;
import org.apache.avro.idl.IdlReader;

/** Writes every named type of an .avdl file as one union schema. */
public final class AvdlUnion {
  private AvdlUnion() {}

  public static void main(String[] args) throws IOException {
    if (args.length != 2) {
      System.err.println("usage: AvdlUnion <in.avdl> <out.avsc>");
      System.exit(2);
    }
    Path in = Paths.get(args[0]);
    var types = new ArrayList<>(new IdlReader().parse(in).getNamedSchemas().values());
    Schema union = Schema.createUnion(types);
    Files.writeString(Paths.get(args[1]), union.toString(), StandardCharsets.UTF_8);
  }
}
