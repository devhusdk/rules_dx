package quality.testdata.jvm_owned;

public final class JavaUnusedDep {
    private JavaUnusedDep() {}

    public static int value() {
        return KeptHelper.value();
    }
}
