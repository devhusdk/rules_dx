import QtQml
import Tally

QtObject {
    property Counter counter: Counter {
        step: 2
        value: 40
    }
    property Badge badge: Badge {
        text: root.counter.label()
    }
    id: root

    function bump() {
        counter.increment();
        badge.text = counter.label();
    }
}
