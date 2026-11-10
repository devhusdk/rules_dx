package dx.avro.tools;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.apache.avro.Schema;
import org.apache.avro.SchemaParseException;
import org.apache.avro.idl.IdlReader;

/** Writes every named type of the given .avsc and .avdl files once, as one union schema; .avdl imports resolve from the working directory. */
public final class AvroUnion {
  private AvroUnion() {}

  public static void main(String[] args) throws IOException {
    if (args.length < 2) {
      System.err.println("usage: AvroUnion <out.avsc> <in.avsc|in.avdl>...");
      System.exit(2);
    }
    Map<String, Schema> known = new LinkedHashMap<>();
    List<Path> pending = new ArrayList<>();
    for (int i = 1; i < args.length; i++) {
      pending.add(Paths.get(args[i]));
    }
    while (!pending.isEmpty()) {
      List<Path> deferred = new ArrayList<>();
      SchemaParseException last = null;
      for (Path in : pending) {
        try {
          known.putAll(parse(in, known));
        } catch (SchemaParseException e) {
          last = e;
          deferred.add(in);
        }
      }
      if (deferred.size() == pending.size()) {
        throw last;
      }
      pending = deferred;
    }
    Schema union = Schema.createUnion(new ArrayList<>(known.values()));
    Files.writeString(Paths.get(args[0]), union.toString(), StandardCharsets.UTF_8);
  }

  private static Map<String, Schema> parse(Path in, Map<String, Schema> known) throws IOException {
    if (in.toString().endsWith(".avdl")) {
      return new IdlReader()
          .parse(Paths.get("").toAbsolutePath().toUri(), Files.readString(in, StandardCharsets.UTF_8))
          .getNamedSchemas();
    }
    Schema.Parser parser = new Schema.Parser();
    parser.addTypes(known.values());
    parser.parse(in.toFile());
    return parser.getTypes();
  }
}
