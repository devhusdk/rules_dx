pragma ComponentBehavior: Bound

import QtQuick
import Tasks.Catalog
import Tasks.Store

Rectangle {
    id: board

    property AssetCatalog catalog: AssetCatalog {}
    property alias current: rows.currentIndex
    property TaskModel tasks: TaskModel {}

    function toggle(row: int): bool {
        if (!tasks.toggle(row)) {
            return false;
        }
        rows.currentIndex = row;
        return tasks.path === "" || tasks.save();
    }

    color: "#ffffff"
    focus: true
    implicitHeight: 24 * (tasks.count + 1)
    implicitWidth: 320

    Keys.onDownPressed: rows.incrementCurrentIndex()
    Keys.onSpacePressed: board.toggle(rows.currentIndex)
    Keys.onUpPressed: rows.decrementCurrentIndex()

    Text {
        color: "#000000"
        font.pixelSize: 16
        height: 24
        text: qsTr("%1 of %2 tasks open").arg(board.tasks.remaining).arg(board.tasks.count)
        verticalAlignment: Text.AlignVCenter
        width: board.width
        x: 4
    }

    ListView {
        id: rows

        anchors.fill: parent
        anchors.topMargin: 24
        interactive: false
        model: board.tasks

        delegate: TaskRow {
            catalog: board.catalog
            current: ListView.isCurrentItem
            width: rows.width

            onToggled: board.toggle(index)
        }
    }
}
