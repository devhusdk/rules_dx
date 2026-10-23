import QtQuick
import QtTest
import Tasks.Board
import Tasks.Store

Item {
    id: stage

    height: 120
    width: 320

    Component {
        id: boardComponent

        Board {}
    }

    Component {
        id: storeComponent

        TaskModel {}
    }

    SignalSpy {
        id: failures

        signalName: "failed"
    }

    TestCase {
        function board(path) {
            const shown = createTemporaryObject(boardComponent, stage);
            shown.tasks.path = path;
            verify(shown.tasks.load());
            shown.tasks.add("Draw logo", "logo");
            shown.tasks.add("Cut trailer", "intro");
            shown.forceActiveFocus();
            waitForRendering(shown);
            return shown;
        }

        function darkPixels(image, width, top, bottom) {
            let dark = 0;
            for (let y = top; y < bottom; ++y) {
                for (let x = 0; x < width; ++x) {
                    dark += image.red(x, y) < 96 ? 1 : 0;
                }
            }
            return dark;
        }

        function init() {
            failures.clear();
        }

        function test_clicking_a_box_repaints_and_saves() {
            const path = scratch + "/click.json";
            const shown = board(path);
            compare(shown.width, 320);
            compare(shown.height, 72);
            let image = grabImage(shown);
            compare(image.red(12, 36), 198);
            compare(image.green(12, 36), 40);
            mouseClick(shown, 12, 36);
            verify(shown.tasks.isDone(0));
            compare(shown.tasks.remaining, 1);
            waitForRendering(shown);
            image = grabImage(shown);
            compare(image.red(12, 36), 46);
            compare(image.green(12, 36), 125);
            compare(image.red(12, 60), 198);
            const reloaded = createTemporaryObject(storeComponent, stage, {
                "path": path
            });
            verify(reloaded.load());
            compare(reloaded.count, 2);
            verify(reloaded.isDone(0));
            verify(!reloaded.isDone(1));
            compare(reloaded.asset(1), "intro");
        }

        function test_keys_move_the_highlight_and_toggle() {
            const shown = board(scratch + "/keys.json");
            compare(shown.current, 0);
            keyClick(Qt.Key_Down);
            compare(shown.current, 1);
            keyClick(Qt.Key_Space);
            verify(!shown.tasks.isDone(0));
            verify(shown.tasks.isDone(1));
            waitForRendering(shown);
            const image = grabImage(shown);
            compare(image.blue(300, 36), 255);
            compare(image.red(300, 36), 255);
            compare(image.red(300, 60), 227);
            compare(image.green(300, 60), 242);
            compare(image.green(12, 60), 125);
            keyClick(Qt.Key_Up);
            keyClick(Qt.Key_Space);
            compare(shown.tasks.remaining, 0);
        }

        function test_text_renders_with_the_pinned_fonts() {
            const shown = board(scratch + "/text.json");
            const image = grabImage(shown);
            verify(darkPixels(image, 200, 0, 24) > 40);
            verify(darkPixels(image, 200, 24, 48) > 40);
            compare(darkPixels(image, 320, 24, 26), 0);
        }

        function test_a_failed_save_is_reported() {
            const shown = board(scratch + "/missing/tasks.json");
            failures.target = shown.tasks;
            mouseClick(shown, 12, 60);
            verify(shown.tasks.isDone(1));
            compare(failures.count, 1);
            verify(failures.signalArguments[0][0].endsWith("/missing/tasks.json: No such file or directory"));
        }

        name: "Board"
        when: windowShown
    }
}
