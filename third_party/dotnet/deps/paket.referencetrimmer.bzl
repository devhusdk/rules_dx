"GENERATED"

load("@rules_dx//dotnet:defs.bzl", "nuget_repo")

def referencetrimmer():
    "referencetrimmer"
    nuget_repo(
        name = "paket.referencetrimmer",
        packages = [
            {"name": "ReferenceTrimmer", "id": "ReferenceTrimmer", "version": "3.5.13", "sha512": "sha512-NpYr/Zcyj/5TjDL1ulf3EK2mfZdZA6auPKFw2usiVjI+m3an+gnrpGibujjPuRKFRNr3fBMgo55ehSpdvid6tQ==", "sources": ["https://api.nuget.org/v3/index.json"], "dependencies": {"net10.0": [], "net11": [], "net20": [], "net30": [], "net35": [], "net40": [], "net403": [], "net45": [], "net451": [], "net452": [], "net46": [], "net461": [], "net462": [], "net47": [], "net471": [], "net472": [], "net48": [], "net5.0": [], "net6.0": [], "net7.0": [], "net8.0": [], "net9.0": [], "netcoreapp1.0": [], "netcoreapp1.1": [], "netcoreapp2.0": [], "netcoreapp2.1": [], "netcoreapp2.2": [], "netcoreapp3.0": [], "netcoreapp3.1": [], "netstandard": [], "netstandard1.0": [], "netstandard1.1": [], "netstandard1.2": [], "netstandard1.3": [], "netstandard1.4": [], "netstandard1.5": [], "netstandard1.6": [], "netstandard2.0": [], "netstandard2.1": []}, "targeting_pack_overrides": [], "framework_list": [], "tools": {}},
        ],
    )
