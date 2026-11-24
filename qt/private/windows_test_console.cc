#ifdef _WIN32
#include <windows.h>

namespace {

// Without a console, Qt shows a blocking MessageBox for a failed platform plugin.
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
