#ifdef _WIN32
#include <windows.h>

namespace {

// Bazel starts Windows tests without a console window, and Qt then shows a
// MessageBox for a failed platform plugin and waits for it. A hidden console
// makes Qt print the error and exit.
struct HiddenConsole {
  HiddenConsole() {
    if (GetConsoleWindow() != nullptr) {
      return;
    }
    const HANDLE in = GetStdHandle(STD_INPUT_HANDLE);
    const HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
    const HANDLE err = GetStdHandle(STD_ERROR_HANDLE);
    FreeConsole();
    AllocConsole();
    ShowWindow(GetConsoleWindow(), SW_HIDE);
    SetStdHandle(STD_INPUT_HANDLE, in);
    SetStdHandle(STD_OUTPUT_HANDLE, out);
    SetStdHandle(STD_ERROR_HANDLE, err);
  }
};

const HiddenConsole hidden_console;

}  // namespace
#endif
