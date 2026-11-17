package org.example.adoptandroid;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

public final class MainActivity extends Activity {
    private static final String TAG = "DxAdoptAndroid";

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        String asset = readAsset("greeting.txt");
        String report = Report.format(
            NativeBridge.summarize(asset),
            getString(R.string.app_name),
            asset,
            "jni");
        for (String line : report.split("\n")) {
            Log.i(TAG, line);
        }
        TextView view = new TextView(this);
        view.setText(report);
        setContentView(view);
    }

    private String readAsset(String name) {
        try (InputStream in = getAssets().open(name)) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[256];
            for (int count = in.read(buffer); count >= 0; count = in.read(buffer)) {
                out.write(buffer, 0, count);
            }
            return new String(out.toByteArray(), StandardCharsets.UTF_8).trim();
        } catch (IOException error) {
            throw new IllegalStateException(error);
        }
    }
}
