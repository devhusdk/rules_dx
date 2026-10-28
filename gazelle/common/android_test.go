package common

import (
	"reflect"
	"strings"
	"testing"
)

const adoptManifest = `<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="org.example.adoptandroid"
    android:versionCode="1"
    android:versionName="1.0">

    <uses-sdk android:minSdkVersion="33" android:targetSdkVersion="33" />

    <uses-permission android:name="android.permission.VIBRATE" />

    <application android:label="@string/app_name">
        <activity android:name="org.example.adoptandroid.MainActivity"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
`

func TestParseAndroidManifest(t *testing.T) {
	ns := `xmlns:android="http://schemas.android.com/apk/res/android"`
	tests := []struct {
		name    string
		in      string
		want    AndroidManifest
		wantErr string
	}{
		{
			name: "adopt-android",
			in:   adoptManifest,
			want: AndroidManifest{Package: "org.example.adoptandroid", MinSDK: 33, TargetSDK: 33, Permissions: []string{"android.permission.VIBRATE"}},
		},
		{
			name: "no uses-sdk",
			in:   `<manifest ` + ns + ` package="a.b"><application/></manifest>`,
			want: AndroidManifest{Package: "a.b"},
		},
		{
			name: "unprefixed sdk ignored",
			in:   `<manifest ` + ns + `><uses-sdk minSdkVersion="21"/></manifest>`,
			want: AndroidManifest{},
		},
		{
			name:    "wrong root",
			in:      `<application/>`,
			wantErr: "want <manifest>",
		},
		{
			name:    "malformed",
			in:      `<manifest><uses-sdk></manifest>`,
			wantErr: "malformed manifest",
		},
		{
			name:    "empty",
			in:      ``,
			wantErr: "no <manifest> root",
		},
		{
			name:    "duplicate uses-sdk",
			in:      `<manifest ` + ns + `><uses-sdk android:minSdkVersion="21"/><uses-sdk android:minSdkVersion="22"/></manifest>`,
			wantErr: "more than one <uses-sdk>",
		},
		{
			name:    "non-integer sdk",
			in:      `<manifest ` + ns + `><uses-sdk android:minSdkVersion="abc"/></manifest>`,
			wantErr: "not an integer",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParseAndroidManifest([]byte(tt.in))
			if tt.wantErr != "" {
				if err == nil || !strings.Contains(err.Error(), tt.wantErr) {
					t.Fatalf("err = %v, want containing %q", err, tt.wantErr)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if !reflect.DeepEqual(got, tt.want) {
				t.Fatalf("got %+v, want %+v", got, tt.want)
			}
		})
	}
}

func TestNDKLib(t *testing.T) {
	tests := []struct {
		header string
		lib    string
		ndk    bool
	}{
		{"android/log.h", "log", true},
		{"android/native_activity.h", "android", true},
		{"android/looper.h", "android", true},
		{"android/input.h", "android", true},
		{"android/asset_manager.h", "android", true},
		{"android/asset_manager_jni.h", "android", true},
		{"android/configuration.h", "android", true},
		{"android/sensor.h", "android", true},
		{"android/native_window.h", "android", true},
		{"android/api-level.h", "", true},
		{"jni.h", "", false},
		{"stdio.h", "", false},
		{"android/", "", false},
	}
	for _, tt := range tests {
		lib, ndk := NDKLib(tt.header)
		if lib != tt.lib || ndk != tt.ndk {
			t.Errorf("NDKLib(%q) = %q, %v; want %q, %v", tt.header, lib, ndk, tt.lib, tt.ndk)
		}
	}
}

func TestIsAndroidSDKPlatform(t *testing.T) {
	for spec, want := range map[string]bool{
		"android.app.Activity":     true,
		"android":                  true,
		"dalvik.system.DexFile":    true,
		"android.support.v4.View":  false,
		"android.arch.lifecycle.X": false,
		"androidx.core.Foo":        false,
		"androidish.Foo":           false,
		"org.json.JSONObject":      false,
	} {
		if got := IsAndroidSDKPlatform(spec); got != want {
			t.Errorf("IsAndroidSDKPlatform(%q) = %v, want %v", spec, got, want)
		}
	}
}

func TestIsAndroidPlatformImport(t *testing.T) {
	for spec, want := range map[string]bool{
		"android.os.Bundle":        true,
		"org.json.JSONObject":      true,
		"org.xmlpull.v1.XmlParser": true,
		"org.apache.http.Client":   false,
		"android.support.v4.View":  false,
		"com.google.common.base.X": false,
	} {
		if got := IsAndroidPlatformImport(spec); got != want {
			t.Errorf("IsAndroidPlatformImport(%q) = %v, want %v", spec, got, want)
		}
	}
}
