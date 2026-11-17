import QtQuick
import Tasks.Catalog

Rectangle {
    id: row

    required property string asset
    property AssetCatalog catalog
    property bool current: false
    required property bool done
    required property int index
    required property string title

    signal toggled

    color: current ? "#e3f2fd" : "#ffffff"
    height: 24

    Rectangle {
        color: row.done ? "#2e7d32" : "#c62828"
        height: 16
        width: 16
        x: 4
        y: 4

        TapHandler {
            onTapped: row.toggled()
        }
    }

    Text {
        color: "#000000"
        font.pixelSize: 14
        font.strikeout: row.done
        height: row.height
        text: row.title + " - " + (row.catalog ? row.catalog.name(row.asset) : row.asset)
        verticalAlignment: Text.AlignVCenter
        x: 28
    }
}
