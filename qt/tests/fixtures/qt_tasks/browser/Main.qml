import QtQml
import QtQml.Models
import Tasks.Catalog
import Tasks.Store

QtObject {
    property AssetCatalog catalog: AssetCatalog {}
    property ListModel history: ListModel {}
    property TaskModel tasks: TaskModel {}

    function add(title: string, asset: string): bool {
        if (!catalog.contains(asset)) {
            history.append({
                "event": "rejected",
                "title": title
            });
            return false;
        }
        tasks.add(title, asset);
        history.append({
            "event": "added",
            "title": title
        });
        return true;
    }

    function line(row: int): string {
        const asset = tasks.asset(row);
        const mark = tasks.isDone(row) ? "[x]" : "[ ]";
        return mark + " " + tasks.title(row) + " - " + qsTranslate("Kinds", catalog.kind(asset)) + " " + catalog.name(asset);
    }

    function summary(): string {
        return qsTr("%1 of %2 tasks open").arg(tasks.remaining).arg(tasks.count);
    }

    function toggle(row: int): bool {
        if (!tasks.toggle(row)) {
            return false;
        }
        history.append({
            "event": tasks.isDone(row) ? "done" : "reopened",
            "title": tasks.title(row)
        });
        return true;
    }
}
