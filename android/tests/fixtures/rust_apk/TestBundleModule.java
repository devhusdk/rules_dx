import com.android.aapt.Resources.Item;
import com.android.aapt.Resources.Primitive;
import com.android.aapt.Resources.XmlAttribute;
import com.android.aapt.Resources.XmlElement;
import com.android.aapt.Resources.XmlNamespace;
import com.android.aapt.Resources.XmlNode;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/** Writes a minimal base module zip with a proto manifest and one native library. */
public final class TestBundleModule {
  private static final String ANDROID = "http://schemas.android.com/apk/res/android";

  private TestBundleModule() {}

  private static XmlAttribute androidInt(String name, int resourceId, int value) {
    return XmlAttribute.newBuilder()
        .setNamespaceUri(ANDROID)
        .setName(name)
        .setResourceId(resourceId)
        .setValue(Integer.toString(value))
        .setCompiledItem(Item.newBuilder().setPrim(Primitive.newBuilder().setIntDecimalValue(value)))
        .build();
  }

  private static XmlAttribute androidString(String name, int resourceId, String value) {
    return XmlAttribute.newBuilder()
        .setNamespaceUri(ANDROID)
        .setName(name)
        .setResourceId(resourceId)
        .setValue(value)
        .build();
  }

  private static XmlAttribute androidBool(String name, int resourceId, boolean value) {
    return XmlAttribute.newBuilder()
        .setNamespaceUri(ANDROID)
        .setName(name)
        .setResourceId(resourceId)
        .setValue(Boolean.toString(value))
        .setCompiledItem(Item.newBuilder().setPrim(Primitive.newBuilder().setBooleanValue(value)))
        .build();
  }

  private static XmlNode element(XmlElement.Builder element) {
    return XmlNode.newBuilder().setElement(element).build();
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 4) {
      System.err.println("usage: TestBundleModule <out.zip> <package> <min_sdk> <abi>");
      System.exit(2);
    }
    XmlNode usesSdk = element(XmlElement.newBuilder()
        .setName("uses-sdk")
        .addAttribute(androidInt("minSdkVersion", 0x0101020c, Integer.parseInt(args[2])))
        .addAttribute(androidInt("targetSdkVersion", 0x01010270, 34)));
    XmlNode application = element(XmlElement.newBuilder()
        .setName("application")
        .addAttribute(androidBool("hasCode", 0x0101000c, false)));
    XmlNode manifest = element(XmlElement.newBuilder()
        .setName("manifest")
        .addNamespaceDeclaration(XmlNamespace.newBuilder().setPrefix("android").setUri(ANDROID))
        .addAttribute(androidInt("versionCode", 0x0101021b, 1))
        .addAttribute(androidString("versionName", 0x0101021c, "1.0"))
        .addAttribute(XmlAttribute.newBuilder().setName("package").setValue(args[1]))
        .addChild(usesSdk)
        .addChild(application));
    try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(args[0]))) {
      zip.putNextEntry(fixed("manifest/AndroidManifest.xml"));
      manifest.writeTo(zip);
      zip.closeEntry();
      zip.putNextEntry(fixed("lib/" + args[3] + "/libstub.so"));
      zip.write("stub".getBytes(StandardCharsets.UTF_8));
      zip.closeEntry();
    }
  }

  private static ZipEntry fixed(String name) {
    ZipEntry entry = new ZipEntry(name);
    entry.setTime(315532800000L);
    return entry;
  }
}
