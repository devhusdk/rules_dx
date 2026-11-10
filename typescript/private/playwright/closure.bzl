"""Pinned Ubuntu 22.04 library closure the Chromium headless shell loads on Linux."""

load("//core:jammy_runtime.bzl", "JAMMY_RUNTIME")

visibility(["//..."])

CLOSURE_DIR = "closure"

_SHARED = ["gcc-12-base", "libgcc-s1", "zlib1g"]

def _shared(platform):
    return [package for package in JAMMY_RUNTIME[platform] if package.package in _SHARED]

CHROMIUM_CLOSURE = {
    "linux_arm64": _shared("linux_arm64") + [
        struct(
            copyright = "usr/share/doc/libasound2/copyright",
            deb = "libasound2_1.2.6.1-1ubuntu1.2_arm64.deb",
            libraries = {
                "libasound.so.2": "usr/lib/aarch64-linux-gnu/libasound.so.2",
            },
            package = "libasound2",
            sha256 = "d9c6a79794e3cb8c46c2a87734a367eeb7f216cca7b58a533e836c33f3020eea",
        ),
        struct(
            copyright = "usr/share/doc/libatk-bridge2.0-0/copyright",
            deb = "libatk-bridge2.0-0_2.38.0-3_arm64.deb",
            libraries = {
                "libatk-bridge-2.0.so.0": "usr/lib/aarch64-linux-gnu/libatk-bridge-2.0.so.0",
            },
            package = "libatk-bridge2.0-0",
            sha256 = "85a170ffcc68a3bc16acb7e520de3e6a8eeaedc3e98e7c32254b502fee03ffbf",
        ),
        struct(
            copyright = "usr/share/doc/libatk1.0-0/copyright",
            deb = "libatk1.0-0_2.36.0-3build1_arm64.deb",
            libraries = {
                "libatk-1.0.so.0": "usr/lib/aarch64-linux-gnu/libatk-1.0.so.0",
            },
            package = "libatk1.0-0",
            sha256 = "c666ec66a5ef79274d14528332aaac5be4aeb5e543d8d0f31b9ad86e64db6dad",
        ),
        struct(
            copyright = "usr/share/doc/libatspi2.0-0/copyright",
            deb = "libatspi2.0-0_2.44.0-3_arm64.deb",
            libraries = {
                "libatspi.so.0": "usr/lib/aarch64-linux-gnu/libatspi.so.0",
            },
            package = "libatspi2.0-0",
            sha256 = "aa0a5a64d6afde3b22053d80a4c450121d18bb2b0b94a41af3584d81081c51f2",
        ),
        struct(
            copyright = "usr/share/doc/libblkid1/copyright",
            deb = "libblkid1_2.37.2-4ubuntu3.6_arm64.deb",
            libraries = {
                "libblkid.so.1": "usr/lib/aarch64-linux-gnu/libblkid.so.1",
            },
            package = "libblkid1",
            sha256 = "2682e7f03f3d0a991e06bd2cb69a69c4cc80b3cdfc12a9a64b6f343b1cc056a3",
        ),
        struct(
            copyright = "usr/share/doc/libbsd0/copyright",
            deb = "libbsd0_0.11.5-1_arm64.deb",
            libraries = {
                "libbsd.so.0": "usr/lib/aarch64-linux-gnu/libbsd.so.0",
            },
            package = "libbsd0",
            sha256 = "19703bfe6fde9a161e42f5107451a46cc182a62dd0621a55d06aec7dc372991d",
        ),
        struct(
            copyright = "usr/share/doc/libcap2/copyright",
            deb = "libcap2_2.44-1ubuntu0.22.04.3_arm64.deb",
            libraries = {
                "libcap.so.2": "lib/aarch64-linux-gnu/libcap.so.2",
            },
            package = "libcap2",
            sha256 = "7a8eb6549355a16f327ede574f12e6ebed983b52ab38ce4a618e6e9274b9d4f5",
        ),
        struct(
            copyright = "usr/share/doc/libdbus-1-3/copyright",
            deb = "libdbus-1-3_1.12.20-2ubuntu4.1_arm64.deb",
            libraries = {
                "libdbus-1.so.3": "lib/aarch64-linux-gnu/libdbus-1.so.3",
            },
            package = "libdbus-1-3",
            sha256 = "2a0c67f581bca221f63db6a6ecfb19da78cd2581f78d7690e6758ca7ca9a656e",
        ),
        struct(
            copyright = "usr/share/doc/libdrm2/copyright",
            deb = "libdrm2_2.4.113-2~ubuntu0.22.04.1_arm64.deb",
            libraries = {
                "libdrm.so.2": "usr/lib/aarch64-linux-gnu/libdrm.so.2",
            },
            package = "libdrm2",
            sha256 = "d61b3eb23638adaed392ae6f48cf9f6ec997d089baecc357663b196c578777b8",
        ),
        struct(
            copyright = "usr/share/doc/libexpat1/copyright",
            deb = "libexpat1_2.4.7-1ubuntu0.9_arm64.deb",
            libraries = {
                "libexpat.so.1": "lib/aarch64-linux-gnu/libexpat.so.1",
            },
            package = "libexpat1",
            sha256 = "b61d2bdf82134f6db0ba5c73d9cfe88805a64cbadbd0d0dffff8410958aab005",
        ),
        struct(
            copyright = "usr/share/doc/libffi8/copyright",
            deb = "libffi8_3.4.2-4_arm64.deb",
            libraries = {
                "libffi.so.8": "usr/lib/aarch64-linux-gnu/libffi.so.8",
            },
            package = "libffi8",
            sha256 = "a0cdf938e8da19bd2252c681f80268609a32d06496a1f8d0ebe1fa41debec697",
        ),
        struct(
            copyright = "usr/share/doc/libgbm1/copyright",
            deb = "libgbm1_23.2.1-1ubuntu3.1~22.04.4_arm64.deb",
            libraries = {
                "libgbm.so.1": "usr/lib/aarch64-linux-gnu/libgbm.so.1",
            },
            package = "libgbm1",
            sha256 = "3cba5492a75b87677b4a81fe2cee27d35ec9795b43edfd9e2b842c7247392df0",
        ),
        struct(
            copyright = "usr/share/doc/libgcrypt20/copyright",
            deb = "libgcrypt20_1.9.4-3ubuntu3.3_arm64.deb",
            libraries = {
                "libgcrypt.so.20": "usr/lib/aarch64-linux-gnu/libgcrypt.so.20",
            },
            package = "libgcrypt20",
            sha256 = "bd98a709641e7e61c8798bb84030886a96fd56424b053a6e9996a8391ce20597",
        ),
        struct(
            copyright = "usr/share/doc/libglib2.0-0/copyright",
            deb = "libglib2.0-0_2.72.4-0ubuntu2.11_arm64.deb",
            libraries = {
                "libgio-2.0.so.0": "usr/lib/aarch64-linux-gnu/libgio-2.0.so.0",
                "libglib-2.0.so.0": "usr/lib/aarch64-linux-gnu/libglib-2.0.so.0",
                "libgmodule-2.0.so.0": "usr/lib/aarch64-linux-gnu/libgmodule-2.0.so.0",
                "libgobject-2.0.so.0": "usr/lib/aarch64-linux-gnu/libgobject-2.0.so.0",
            },
            package = "libglib2.0-0",
            sha256 = "dd637fc85cc9d9c0b268925761edc5602ef2e0c0639e36aab4971771e4c4951f",
        ),
        struct(
            copyright = "usr/share/doc/libgpg-error0/copyright",
            deb = "libgpg-error0_1.43-3_arm64.deb",
            libraries = {
                "libgpg-error.so.0": "lib/aarch64-linux-gnu/libgpg-error.so.0",
            },
            package = "libgpg-error0",
            sha256 = "ffa9029cdeb2786e0db1f614f3d2205f75f466a66b25e22aa89ecdf814491875",
        ),
        struct(
            copyright = "usr/share/doc/liblz4-1/copyright",
            deb = "liblz4-1_1.9.3-2build2_arm64.deb",
            libraries = {
                "liblz4.so.1": "usr/lib/aarch64-linux-gnu/liblz4.so.1",
            },
            package = "liblz4-1",
            sha256 = "3e2274893e0963c758f0d18ba80d033736909373247a7deb211e9ff590a09133",
        ),
        struct(
            copyright = "usr/share/doc/liblzma5/copyright",
            deb = "liblzma5_5.2.5-2ubuntu1.2_arm64.deb",
            libraries = {
                "liblzma.so.5": "lib/aarch64-linux-gnu/liblzma.so.5",
            },
            package = "liblzma5",
            sha256 = "e98ef4ff8c23c34fd2fd13a341d69fd1240a0af790682c3afb93dac6c87d995a",
        ),
        struct(
            copyright = "usr/share/doc/libmd0/copyright",
            deb = "libmd0_1.0.4-1build1_arm64.deb",
            libraries = {
                "libmd.so.0": "usr/lib/aarch64-linux-gnu/libmd.so.0",
            },
            package = "libmd0",
            sha256 = "6f87f19f71411a94ae370f7f9bcf4c7dfb5d4393f11de99634ac3137448ad678",
        ),
        struct(
            copyright = "usr/share/doc/libmount1/copyright",
            deb = "libmount1_2.37.2-4ubuntu3.6_arm64.deb",
            libraries = {
                "libmount.so.1": "usr/lib/aarch64-linux-gnu/libmount.so.1",
            },
            package = "libmount1",
            sha256 = "a0a733b6fe171af62dc034eebe439ba680a108ae23e518c637e7adfe42236c01",
        ),
        struct(
            copyright = "usr/share/doc/libnspr4/copyright",
            deb = "libnspr4_4.35-0ubuntu0.22.04.1_arm64.deb",
            libraries = {
                "libnspr4.so": "usr/lib/aarch64-linux-gnu/libnspr4.so",
                "libplc4.so": "usr/lib/aarch64-linux-gnu/libplc4.so",
                "libplds4.so": "usr/lib/aarch64-linux-gnu/libplds4.so",
            },
            package = "libnspr4",
            sha256 = "0a256ba7ad57f2ff418fca9c7a004ecc5e1fd277bbbafc66cdb712d19019e58e",
        ),
        struct(
            copyright = "usr/share/doc/libnss3/copyright",
            deb = "libnss3_3.98-0ubuntu0.22.04.5_arm64.deb",
            libraries = {
                "libfreebl3.so": "usr/lib/aarch64-linux-gnu/libfreebl3.so",
                "libfreeblpriv3.so": "usr/lib/aarch64-linux-gnu/libfreeblpriv3.so",
                "libnss3.so": "usr/lib/aarch64-linux-gnu/libnss3.so",
                "libnssckbi.so": "usr/lib/aarch64-linux-gnu/nss/libnssckbi.so",
                "libnssdbm3.so": "usr/lib/aarch64-linux-gnu/nss/libnssdbm3.so",
                "libnssutil3.so": "usr/lib/aarch64-linux-gnu/libnssutil3.so",
                "libsoftokn3.so": "usr/lib/aarch64-linux-gnu/nss/libsoftokn3.so",
            },
            package = "libnss3",
            sha256 = "f4909fc604b3556e1f70e0a108d6589a59212abfad78290d80a8e36fa1a105a3",
        ),
        struct(
            copyright = "usr/share/doc/libpcre2-8-0/copyright",
            deb = "libpcre2-8-0_10.39-3ubuntu0.2_arm64.deb",
            libraries = {
                "libpcre2-8.so.0": "usr/lib/aarch64-linux-gnu/libpcre2-8.so.0",
            },
            package = "libpcre2-8-0",
            sha256 = "fe4e89a79c959dc7df5f84133490c39230ada2f2949aa20638cfecbb1a0c52c4",
        ),
        struct(
            copyright = "usr/share/doc/libpcre3/copyright",
            deb = "libpcre3_8.39-13ubuntu0.22.04.1_arm64.deb",
            libraries = {
                "libpcre.so.3": "lib/aarch64-linux-gnu/libpcre.so.3",
            },
            package = "libpcre3",
            sha256 = "ff6766bf08ccdb87b684339cceeb64a35bb34412b4978d7d0798a9bca6b71a22",
        ),
        struct(
            copyright = "usr/share/doc/libselinux1/copyright",
            deb = "libselinux1_3.3-1build2_arm64.deb",
            libraries = {
                "libselinux.so.1": "lib/aarch64-linux-gnu/libselinux.so.1",
            },
            package = "libselinux1",
            sha256 = "3a0726b2174a40fa28e74712867aa6e40d3d725543e8f668af9e9e59a28c817c",
        ),
        struct(
            copyright = "usr/share/doc/libsqlite3-0/copyright",
            deb = "libsqlite3-0_3.37.2-2ubuntu0.8_arm64.deb",
            libraries = {
                "libsqlite3.so.0": "usr/lib/aarch64-linux-gnu/libsqlite3.so.0",
            },
            package = "libsqlite3-0",
            sha256 = "0bb81a51a7a56aa66a6bf981e6a4dc4040b1873dc2fc29e8834c977585068e9c",
        ),
        struct(
            copyright = "usr/share/doc/libsystemd0/copyright",
            deb = "libsystemd0_249.11-0ubuntu3.23_arm64.deb",
            libraries = {
                "libsystemd.so.0": "usr/lib/aarch64-linux-gnu/libsystemd.so.0",
            },
            package = "libsystemd0",
            sha256 = "a50379fcab4a6b974c5ee983ab2abd4341768a857715f2e8b855d3610dd73389",
        ),
        struct(
            copyright = "usr/share/doc/libudev1/copyright",
            deb = "libudev1_249.11-0ubuntu3.23_arm64.deb",
            libraries = {
                "libudev.so.1": "usr/lib/aarch64-linux-gnu/libudev.so.1",
            },
            package = "libudev1",
            sha256 = "23f6d546ce43012ddc5b24bf9428fd74be20e161f90058287af612f590e82731",
        ),
        struct(
            copyright = "usr/share/doc/libwayland-server0/copyright",
            deb = "libwayland-server0_1.20.0-1ubuntu0.1_arm64.deb",
            libraries = {
                "libwayland-server.so.0": "usr/lib/aarch64-linux-gnu/libwayland-server.so.0",
            },
            package = "libwayland-server0",
            sha256 = "fb81c068a3b98ca012a21a71c1b031e5d16c5a8423cb112ac86aec8eef556d49",
        ),
        struct(
            copyright = "usr/share/doc/libx11-6/copyright",
            deb = "libx11-6_1.7.5-1ubuntu0.4_arm64.deb",
            libraries = {
                "libX11.so.6": "usr/lib/aarch64-linux-gnu/libX11.so.6",
            },
            package = "libx11-6",
            sha256 = "1b78cb4bc6a5020a1a19025d5cad06fee5187cef5e06ee0e9e55940b366d4197",
        ),
        struct(
            copyright = "usr/share/doc/libxau6/copyright",
            deb = "libxau6_1.0.9-1build5_arm64.deb",
            libraries = {
                "libXau.so.6": "usr/lib/aarch64-linux-gnu/libXau.so.6",
            },
            package = "libxau6",
            sha256 = "077ac2db5df32b02e68e3d4de0b4cc11e0114bf2ee8a94866a437757d1722d92",
        ),
        struct(
            copyright = "usr/share/doc/libxcb-randr0/copyright",
            deb = "libxcb-randr0_1.14-3ubuntu3_arm64.deb",
            libraries = {
                "libxcb-randr.so.0": "usr/lib/aarch64-linux-gnu/libxcb-randr.so.0",
            },
            package = "libxcb-randr0",
            sha256 = "93096f58069737b07101b00310aea8c0b82c1321053b052475a868d2b059a54e",
        ),
        struct(
            copyright = "usr/share/doc/libxcb1/copyright",
            deb = "libxcb1_1.14-3ubuntu3_arm64.deb",
            libraries = {
                "libxcb.so.1": "usr/lib/aarch64-linux-gnu/libxcb.so.1",
            },
            package = "libxcb1",
            sha256 = "0774d9a96623001eb0bb2d6931b773655deb86de43d4c0840ec5f1dfa85c6810",
        ),
        struct(
            copyright = "usr/share/doc/libxcomposite1/copyright",
            deb = "libxcomposite1_0.4.5-1build2_arm64.deb",
            libraries = {
                "libXcomposite.so.1": "usr/lib/aarch64-linux-gnu/libXcomposite.so.1",
            },
            package = "libxcomposite1",
            sha256 = "35b036c97fee61df4981d9e3e08a26e3110a41b30992bd5e11b0cf04c1e16b47",
        ),
        struct(
            copyright = "usr/share/doc/libxdamage1/copyright",
            deb = "libxdamage1_1.1.5-2build2_arm64.deb",
            libraries = {
                "libXdamage.so.1": "usr/lib/aarch64-linux-gnu/libXdamage.so.1",
            },
            package = "libxdamage1",
            sha256 = "8075485c8a60869c7490834beb3a40ee2f448b94cab00b1dca08e8103d5c5514",
        ),
        struct(
            copyright = "usr/share/doc/libxdmcp6/copyright",
            deb = "libxdmcp6_1.1.3-0ubuntu5_arm64.deb",
            libraries = {
                "libXdmcp.so.6": "usr/lib/aarch64-linux-gnu/libXdmcp.so.6",
            },
            package = "libxdmcp6",
            sha256 = "aa2a89a256c30844db3831e0b027d31c0d3a9f0e3108aa7dd1310a80fa21b453",
        ),
        struct(
            copyright = "usr/share/doc/libxext6/copyright",
            deb = "libxext6_1.3.4-1build1_arm64.deb",
            libraries = {
                "libXext.so.6": "usr/lib/aarch64-linux-gnu/libXext.so.6",
            },
            package = "libxext6",
            sha256 = "8abb17f19b47e22e0e85b85e7c59ed0cba5faea335e18eccbe499242608e984f",
        ),
        struct(
            copyright = "usr/share/doc/libxfixes3/copyright",
            deb = "libxfixes3_6.0.0-1_arm64.deb",
            libraries = {
                "libXfixes.so.3": "usr/lib/aarch64-linux-gnu/libXfixes.so.3",
            },
            package = "libxfixes3",
            sha256 = "6de064af315c0aa15d299e390583fce689d58e3e463b57c0fd35804c86c52d4a",
        ),
        struct(
            copyright = "usr/share/doc/libxi6/copyright",
            deb = "libxi6_1.8-1build1_arm64.deb",
            libraries = {
                "libXi.so.6": "usr/lib/aarch64-linux-gnu/libXi.so.6",
            },
            package = "libxi6",
            sha256 = "c39433b0046ed2a7151eb772b3fb4c6588ecd454328a40b3688574d0d52c9512",
        ),
        struct(
            copyright = "usr/share/doc/libxkbcommon0/copyright",
            deb = "libxkbcommon0_1.4.0-1_arm64.deb",
            libraries = {
                "libxkbcommon.so.0": "usr/lib/aarch64-linux-gnu/libxkbcommon.so.0",
            },
            package = "libxkbcommon0",
            sha256 = "451be798e301077cde9d34afe00886e6c77699b1734985c2191a7a4ba783911d",
        ),
        struct(
            copyright = "usr/share/doc/libxrandr2/copyright",
            deb = "libxrandr2_1.5.2-1build1_arm64.deb",
            libraries = {
                "libXrandr.so.2": "usr/lib/aarch64-linux-gnu/libXrandr.so.2",
            },
            package = "libxrandr2",
            sha256 = "6edc5451bbdfad37714c33eab557bbace9e0f796ed817b37d56c0b9d091da516",
        ),
        struct(
            copyright = "usr/share/doc/libxrender1/copyright",
            deb = "libxrender1_0.9.10-1build4_arm64.deb",
            libraries = {
                "libXrender.so.1": "usr/lib/aarch64-linux-gnu/libXrender.so.1",
            },
            package = "libxrender1",
            sha256 = "e9a67645ce73ac119f5ebbf95ea42de5a39dddc03f57bfbacb28d82419d3e6b1",
        ),
        struct(
            copyright = "usr/share/doc/libzstd1/copyright",
            deb = "libzstd1_1.4.8+dfsg-3build1_arm64.deb",
            libraries = {
                "libzstd.so.1": "usr/lib/aarch64-linux-gnu/libzstd.so.1",
            },
            package = "libzstd1",
            sha256 = "0102eb80384c2432c4be819663341a1c4e511d05f13ec1467643696531ec6491",
        ),
    ],
    "linux_x86_64": _shared("linux_x86_64") + [
        struct(
            copyright = "usr/share/doc/libasound2/copyright",
            deb = "libasound2_1.2.6.1-1ubuntu1.2_amd64.deb",
            libraries = {
                "libasound.so.2": "usr/lib/x86_64-linux-gnu/libasound.so.2",
            },
            package = "libasound2",
            sha256 = "dce4ce1043cde35f4bc375a4b1bd84badd5dec7e656ea2a8849edf9215bc8c33",
        ),
        struct(
            copyright = "usr/share/doc/libatk-bridge2.0-0/copyright",
            deb = "libatk-bridge2.0-0_2.38.0-3_amd64.deb",
            libraries = {
                "libatk-bridge-2.0.so.0": "usr/lib/x86_64-linux-gnu/libatk-bridge-2.0.so.0",
            },
            package = "libatk-bridge2.0-0",
            sha256 = "98924a5b376da9c075663ea61721fb0d91a55be3473c4643286491247f229b48",
        ),
        struct(
            copyright = "usr/share/doc/libatk1.0-0/copyright",
            deb = "libatk1.0-0_2.36.0-3build1_amd64.deb",
            libraries = {
                "libatk-1.0.so.0": "usr/lib/x86_64-linux-gnu/libatk-1.0.so.0",
            },
            package = "libatk1.0-0",
            sha256 = "598b522b64a20c9a3ea44fa816b539976c39d41bf2bccd104fed49954638f0f9",
        ),
        struct(
            copyright = "usr/share/doc/libatspi2.0-0/copyright",
            deb = "libatspi2.0-0_2.44.0-3_amd64.deb",
            libraries = {
                "libatspi.so.0": "usr/lib/x86_64-linux-gnu/libatspi.so.0",
            },
            package = "libatspi2.0-0",
            sha256 = "93b243e9d8fb3de9ba78251cf3dec2c6b66836ea7329ae4026981ce8dda62de4",
        ),
        struct(
            copyright = "usr/share/doc/libblkid1/copyright",
            deb = "libblkid1_2.37.2-4ubuntu3.6_amd64.deb",
            libraries = {
                "libblkid.so.1": "usr/lib/x86_64-linux-gnu/libblkid.so.1",
            },
            package = "libblkid1",
            sha256 = "bf9ab18457e0a10879951b01c87156bdf503bdeb0f87ae0f0f81ec62cb000279",
        ),
        struct(
            copyright = "usr/share/doc/libbsd0/copyright",
            deb = "libbsd0_0.11.5-1_amd64.deb",
            libraries = {
                "libbsd.so.0": "usr/lib/x86_64-linux-gnu/libbsd.so.0",
            },
            package = "libbsd0",
            sha256 = "09367acdf59f28ffb71ba7bc36c516cff03b2765930cf9be7bb7ea7273e4c312",
        ),
        struct(
            copyright = "usr/share/doc/libcap2/copyright",
            deb = "libcap2_2.44-1ubuntu0.22.04.3_amd64.deb",
            libraries = {
                "libcap.so.2": "lib/x86_64-linux-gnu/libcap.so.2",
            },
            package = "libcap2",
            sha256 = "c497efb7fa3cd19a7ff297d95c7c5d678063c80c9fc230c9d034a8ee02ca8446",
        ),
        struct(
            copyright = "usr/share/doc/libdbus-1-3/copyright",
            deb = "libdbus-1-3_1.12.20-2ubuntu4.1_amd64.deb",
            libraries = {
                "libdbus-1.so.3": "lib/x86_64-linux-gnu/libdbus-1.so.3",
            },
            package = "libdbus-1-3",
            sha256 = "6a5171696c3b05aca273d637d9c0501c8033fda64a971e09fd545bc1ab72ca1e",
        ),
        struct(
            copyright = "usr/share/doc/libdrm2/copyright",
            deb = "libdrm2_2.4.113-2~ubuntu0.22.04.1_amd64.deb",
            libraries = {
                "libdrm.so.2": "usr/lib/x86_64-linux-gnu/libdrm.so.2",
            },
            package = "libdrm2",
            sha256 = "555925f1caedfd98d89bae3bc11b4fe3401c2c4817dbbafcab25e1144582e946",
        ),
        struct(
            copyright = "usr/share/doc/libexpat1/copyright",
            deb = "libexpat1_2.4.7-1ubuntu0.9_amd64.deb",
            libraries = {
                "libexpat.so.1": "lib/x86_64-linux-gnu/libexpat.so.1",
            },
            package = "libexpat1",
            sha256 = "0ee31596316d2275ab351d11290c63e7547fff0ecd7f431e56855cb7c5f18588",
        ),
        struct(
            copyright = "usr/share/doc/libffi8/copyright",
            deb = "libffi8_3.4.2-4_amd64.deb",
            libraries = {
                "libffi.so.8": "usr/lib/x86_64-linux-gnu/libffi.so.8",
            },
            package = "libffi8",
            sha256 = "b4f88c91fa6f4c942097be6abfc365fb133c5e147640168cbb7704fd855eac10",
        ),
        struct(
            copyright = "usr/share/doc/libgbm1/copyright",
            deb = "libgbm1_23.2.1-1ubuntu3.1~22.04.4_amd64.deb",
            libraries = {
                "libgbm.so.1": "usr/lib/x86_64-linux-gnu/libgbm.so.1",
            },
            package = "libgbm1",
            sha256 = "10d37ffd63f8f98fc9a1ad5b32abc7a9da760f4ed85c806d98f10a61c17f8ef5",
        ),
        struct(
            copyright = "usr/share/doc/libgcrypt20/copyright",
            deb = "libgcrypt20_1.9.4-3ubuntu3.3_amd64.deb",
            libraries = {
                "libgcrypt.so.20": "usr/lib/x86_64-linux-gnu/libgcrypt.so.20",
            },
            package = "libgcrypt20",
            sha256 = "8f16329a0aabf4900271a3687890721bbef307e60983acae5b92eeff12e799b3",
        ),
        struct(
            copyright = "usr/share/doc/libglib2.0-0/copyright",
            deb = "libglib2.0-0_2.72.4-0ubuntu2.11_amd64.deb",
            libraries = {
                "libgio-2.0.so.0": "usr/lib/x86_64-linux-gnu/libgio-2.0.so.0",
                "libglib-2.0.so.0": "usr/lib/x86_64-linux-gnu/libglib-2.0.so.0",
                "libgmodule-2.0.so.0": "usr/lib/x86_64-linux-gnu/libgmodule-2.0.so.0",
                "libgobject-2.0.so.0": "usr/lib/x86_64-linux-gnu/libgobject-2.0.so.0",
            },
            package = "libglib2.0-0",
            sha256 = "78352d59576a9d6113163a53e51dc4d521315e50f983817eac54eaf2f20fd018",
        ),
        struct(
            copyright = "usr/share/doc/libgpg-error0/copyright",
            deb = "libgpg-error0_1.43-3_amd64.deb",
            libraries = {
                "libgpg-error.so.0": "lib/x86_64-linux-gnu/libgpg-error.so.0",
            },
            package = "libgpg-error0",
            sha256 = "1fbacdf9bd1e431cee874a697b339f6f925182bc79bba5a112b53669b33265c5",
        ),
        struct(
            copyright = "usr/share/doc/liblz4-1/copyright",
            deb = "liblz4-1_1.9.3-2build2_amd64.deb",
            libraries = {
                "liblz4.so.1": "usr/lib/x86_64-linux-gnu/liblz4.so.1",
            },
            package = "liblz4-1",
            sha256 = "ac9b54d0feb840345060c74fb687675c5e1eb2b195effafae38c5f9991041e98",
        ),
        struct(
            copyright = "usr/share/doc/liblzma5/copyright",
            deb = "liblzma5_5.2.5-2ubuntu1.2_amd64.deb",
            libraries = {
                "liblzma.so.5": "lib/x86_64-linux-gnu/liblzma.so.5",
            },
            package = "liblzma5",
            sha256 = "a88fca7506963c443c42a578db6c35424336ae28abcdaa60c89f06d30aeb625b",
        ),
        struct(
            copyright = "usr/share/doc/libmd0/copyright",
            deb = "libmd0_1.0.4-1build1_amd64.deb",
            libraries = {
                "libmd.so.0": "usr/lib/x86_64-linux-gnu/libmd.so.0",
            },
            package = "libmd0",
            sha256 = "bcdad5e400aeb660f0337a1c5c1a77e3effa91d0803e64ce2d000e9006f67cdb",
        ),
        struct(
            copyright = "usr/share/doc/libmount1/copyright",
            deb = "libmount1_2.37.2-4ubuntu3.6_amd64.deb",
            libraries = {
                "libmount.so.1": "usr/lib/x86_64-linux-gnu/libmount.so.1",
            },
            package = "libmount1",
            sha256 = "9f69b37420731477105d94c6bf2139437be04bef7c46ba7ffcaf3741e301bdba",
        ),
        struct(
            copyright = "usr/share/doc/libnspr4/copyright",
            deb = "libnspr4_4.35-0ubuntu0.22.04.1_amd64.deb",
            libraries = {
                "libnspr4.so": "usr/lib/x86_64-linux-gnu/libnspr4.so",
                "libplc4.so": "usr/lib/x86_64-linux-gnu/libplc4.so",
                "libplds4.so": "usr/lib/x86_64-linux-gnu/libplds4.so",
            },
            package = "libnspr4",
            sha256 = "b3c96e4a61675c87f8d9655109346748847d859abc95f20493159d06b5aa30ef",
        ),
        struct(
            copyright = "usr/share/doc/libnss3/copyright",
            deb = "libnss3_3.98-0ubuntu0.22.04.5_amd64.deb",
            libraries = {
                "libfreebl3.so": "usr/lib/x86_64-linux-gnu/libfreebl3.so",
                "libfreeblpriv3.so": "usr/lib/x86_64-linux-gnu/libfreeblpriv3.so",
                "libnss3.so": "usr/lib/x86_64-linux-gnu/libnss3.so",
                "libnssckbi.so": "usr/lib/x86_64-linux-gnu/nss/libnssckbi.so",
                "libnssdbm3.so": "usr/lib/x86_64-linux-gnu/nss/libnssdbm3.so",
                "libnssutil3.so": "usr/lib/x86_64-linux-gnu/libnssutil3.so",
                "libsoftokn3.so": "usr/lib/x86_64-linux-gnu/nss/libsoftokn3.so",
            },
            package = "libnss3",
            sha256 = "75f3879ab9969a930b5bf4b9229345d57e2ebe310ad8ce58c9d91393d823b236",
        ),
        struct(
            copyright = "usr/share/doc/libpcre2-8-0/copyright",
            deb = "libpcre2-8-0_10.39-3ubuntu0.2_amd64.deb",
            libraries = {
                "libpcre2-8.so.0": "usr/lib/x86_64-linux-gnu/libpcre2-8.so.0",
            },
            package = "libpcre2-8-0",
            sha256 = "30dff428411080f04c17d3146cecff946bed94f019f912ea3aa1d2630dee610f",
        ),
        struct(
            copyright = "usr/share/doc/libpcre3/copyright",
            deb = "libpcre3_8.39-13ubuntu0.22.04.1_amd64.deb",
            libraries = {
                "libpcre.so.3": "lib/x86_64-linux-gnu/libpcre.so.3",
            },
            package = "libpcre3",
            sha256 = "0d1eb051047eb0c637c893f618f45e1d419863bfa3bc821897ee05ad418ba22e",
        ),
        struct(
            copyright = "usr/share/doc/libselinux1/copyright",
            deb = "libselinux1_3.3-1build2_amd64.deb",
            libraries = {
                "libselinux.so.1": "lib/x86_64-linux-gnu/libselinux.so.1",
            },
            package = "libselinux1",
            sha256 = "b96c6b40ee2388bd51341cb11c0f1d5bcca29b9180b6e3a77a06b881f2913f7e",
        ),
        struct(
            copyright = "usr/share/doc/libsqlite3-0/copyright",
            deb = "libsqlite3-0_3.37.2-2ubuntu0.8_amd64.deb",
            libraries = {
                "libsqlite3.so.0": "usr/lib/x86_64-linux-gnu/libsqlite3.so.0",
            },
            package = "libsqlite3-0",
            sha256 = "6e22b1e8d375fed6a1d52cad5887e762b6cdeaba137595e726c95486968dfab5",
        ),
        struct(
            copyright = "usr/share/doc/libsystemd0/copyright",
            deb = "libsystemd0_249.11-0ubuntu3.23_amd64.deb",
            libraries = {
                "libsystemd.so.0": "usr/lib/x86_64-linux-gnu/libsystemd.so.0",
            },
            package = "libsystemd0",
            sha256 = "ff02a85ba73f850cf7958d2867b34947c67a1072e2b2d1c108a8c6e23715624d",
        ),
        struct(
            copyright = "usr/share/doc/libudev1/copyright",
            deb = "libudev1_249.11-0ubuntu3.23_amd64.deb",
            libraries = {
                "libudev.so.1": "usr/lib/x86_64-linux-gnu/libudev.so.1",
            },
            package = "libudev1",
            sha256 = "e6bf821bc82eb05ba44692242a4da43b34b65ce137c0c4a46986bb122a4624ef",
        ),
        struct(
            copyright = "usr/share/doc/libwayland-server0/copyright",
            deb = "libwayland-server0_1.20.0-1ubuntu0.1_amd64.deb",
            libraries = {
                "libwayland-server.so.0": "usr/lib/x86_64-linux-gnu/libwayland-server.so.0",
            },
            package = "libwayland-server0",
            sha256 = "b5d514dc17edac6b93ba2476d90806337ca4afcc914952a7e78af2e5cd0c4a7b",
        ),
        struct(
            copyright = "usr/share/doc/libx11-6/copyright",
            deb = "libx11-6_1.7.5-1ubuntu0.4_amd64.deb",
            libraries = {
                "libX11.so.6": "usr/lib/x86_64-linux-gnu/libX11.so.6",
            },
            package = "libx11-6",
            sha256 = "6fcad3c4020a87caad67cc4141a82372ed474c050e6eeb9d434594590760430e",
        ),
        struct(
            copyright = "usr/share/doc/libxau6/copyright",
            deb = "libxau6_1.0.9-1build5_amd64.deb",
            libraries = {
                "libXau.so.6": "usr/lib/x86_64-linux-gnu/libXau.so.6",
            },
            package = "libxau6",
            sha256 = "551fa4101394af894f5f401285eeca9756da94bb31ecb08ed36a549ec580cd10",
        ),
        struct(
            copyright = "usr/share/doc/libxcb-randr0/copyright",
            deb = "libxcb-randr0_1.14-3ubuntu3_amd64.deb",
            libraries = {
                "libxcb-randr.so.0": "usr/lib/x86_64-linux-gnu/libxcb-randr.so.0",
            },
            package = "libxcb-randr0",
            sha256 = "39dc0f73fa3bd43f3f7755cb7d12bfd1cd59dd8966f747d66fba47dfc8652f62",
        ),
        struct(
            copyright = "usr/share/doc/libxcb1/copyright",
            deb = "libxcb1_1.14-3ubuntu3_amd64.deb",
            libraries = {
                "libxcb.so.1": "usr/lib/x86_64-linux-gnu/libxcb.so.1",
            },
            package = "libxcb1",
            sha256 = "9258f04136434e7a0ccd76041505421934284209693373cfa9c24acb3a641ec5",
        ),
        struct(
            copyright = "usr/share/doc/libxcomposite1/copyright",
            deb = "libxcomposite1_0.4.5-1build2_amd64.deb",
            libraries = {
                "libXcomposite.so.1": "usr/lib/x86_64-linux-gnu/libXcomposite.so.1",
            },
            package = "libxcomposite1",
            sha256 = "1e3d7e7b53149f6fff92e1fbf4203b61301ca4e251b2949a34bf788dd999621f",
        ),
        struct(
            copyright = "usr/share/doc/libxdamage1/copyright",
            deb = "libxdamage1_1.1.5-2build2_amd64.deb",
            libraries = {
                "libXdamage.so.1": "usr/lib/x86_64-linux-gnu/libXdamage.so.1",
            },
            package = "libxdamage1",
            sha256 = "a705af86334b9c219e5f50e21e5bc51bfbf8b8189ca88d4fcafff734de81a503",
        ),
        struct(
            copyright = "usr/share/doc/libxdmcp6/copyright",
            deb = "libxdmcp6_1.1.3-0ubuntu5_amd64.deb",
            libraries = {
                "libXdmcp.so.6": "usr/lib/x86_64-linux-gnu/libXdmcp.so.6",
            },
            package = "libxdmcp6",
            sha256 = "6f6c869dcfc072fa77d24b72ba127bdf15d606bdffbeba4314fb39f4f324363d",
        ),
        struct(
            copyright = "usr/share/doc/libxext6/copyright",
            deb = "libxext6_1.3.4-1build1_amd64.deb",
            libraries = {
                "libXext.so.6": "usr/lib/x86_64-linux-gnu/libXext.so.6",
            },
            package = "libxext6",
            sha256 = "875ec849749511dff61c8cc5a775f6f93427b9c3f392fdcec6925f14428e89cd",
        ),
        struct(
            copyright = "usr/share/doc/libxfixes3/copyright",
            deb = "libxfixes3_6.0.0-1_amd64.deb",
            libraries = {
                "libXfixes.so.3": "usr/lib/x86_64-linux-gnu/libXfixes.so.3",
            },
            package = "libxfixes3",
            sha256 = "f627555440060edf19801d77d369c99547cf47123fe4e30c5bdb8b664f0b4467",
        ),
        struct(
            copyright = "usr/share/doc/libxi6/copyright",
            deb = "libxi6_1.8-1build1_amd64.deb",
            libraries = {
                "libXi.so.6": "usr/lib/x86_64-linux-gnu/libXi.so.6",
            },
            package = "libxi6",
            sha256 = "47662736cca4be456fd42a5f0a9d3bed8a5cfad4ee08b809a95dde6004daf275",
        ),
        struct(
            copyright = "usr/share/doc/libxkbcommon0/copyright",
            deb = "libxkbcommon0_1.4.0-1_amd64.deb",
            libraries = {
                "libxkbcommon.so.0": "usr/lib/x86_64-linux-gnu/libxkbcommon.so.0",
            },
            package = "libxkbcommon0",
            sha256 = "a0470edb237d52b71ad41f755c043164abed01423585ed5ea19525b3baaf84f7",
        ),
        struct(
            copyright = "usr/share/doc/libxrandr2/copyright",
            deb = "libxrandr2_1.5.2-1build1_amd64.deb",
            libraries = {
                "libXrandr.so.2": "usr/lib/x86_64-linux-gnu/libXrandr.so.2",
            },
            package = "libxrandr2",
            sha256 = "5fbce1f1ec150c715fd83eeef5626363580218db09cedc8ac888b172476b0c82",
        ),
        struct(
            copyright = "usr/share/doc/libxrender1/copyright",
            deb = "libxrender1_0.9.10-1build4_amd64.deb",
            libraries = {
                "libXrender.so.1": "usr/lib/x86_64-linux-gnu/libXrender.so.1",
            },
            package = "libxrender1",
            sha256 = "a68f6db4320b5963dd499b83b6cc8c7b5cd39b05e84862d5945da9244e4c96d6",
        ),
        struct(
            copyright = "usr/share/doc/libzstd1/copyright",
            deb = "libzstd1_1.4.8+dfsg-3build1_amd64.deb",
            libraries = {
                "libzstd.so.1": "usr/lib/x86_64-linux-gnu/libzstd.so.1",
            },
            package = "libzstd1",
            sha256 = "ae7db00ce8b093e50c994518b90203544e063b4bc574836a048bb142b950b2c9",
        ),
    ],
}
