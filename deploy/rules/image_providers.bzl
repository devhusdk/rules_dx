"""Providers of the OCI image quality wrapper."""

OciImageQualityInfo = provider(
    doc = "The OCI layout directory and name of an image that dx lint scans.",
    fields = {
        "layout": "File: the OCI image layout directory the image rule produced.",
        "name": "string: target name of the image.",
    },
)
