import QtQuick
import QtTest
import Tally

Rectangle {
    id: swatch

    property Counter counter: Counter {
        step: 5
    }

    color: counter.value > 0 ? "#00ff00" : "#ff0000"
    height: 20
    width: 20

    TestCase {
        name: "Tally"
        when: windowShown

        function test_counts() {
            swatch.counter.value = 0;
            swatch.counter.increment();
            compare(swatch.counter.label(), "tally 5");
        }

        function test_renders() {
            swatch.counter.value = 1;
            waitForRendering(swatch);
            const image = grabImage(swatch);
            compare(image.red(10, 10), 0);
            compare(image.green(10, 10), 255);
        }
    }
}
