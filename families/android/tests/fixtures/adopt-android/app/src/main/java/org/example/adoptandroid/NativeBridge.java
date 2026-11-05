package org.example.adoptandroid;

public final class NativeBridge {
    private NativeBridge() {}

    static {
        System.loadLibrary("adopt_apk_bin");
    }

    public static native String summarize(String text);
}
