import QtQml
import Ledger.Model

QtObject {
    id: root

    property LedgerModel ledger: LedgerModel {}
    property list<int> rejections: []
    property Connections watcher: Connections {
        function onRejected(amount: int) {
            root.rejections.push(amount);
        }

        target: root.ledger
    }

    function deposit(amount: int): bool {
        return ledger.add(amount);
    }

    function summary(): string {
        return ledger.count + " entries, total " + ledger.total + ", adler32 " + ledger.checksum.toString(16).padStart(8, "0");
    }
}
