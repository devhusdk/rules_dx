package tests.fixtures.fuzz;

public class ParseFuzz {
    public static void fuzzerTestOneInput(byte[] data) {
        if (data.length >= 3 && data[0] == 'f' && data[1] == 'u' && data[2] == 'z') {
            throw new IllegalStateException("planted crash");
        }
    }
}
