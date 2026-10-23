#pragma once

#include <QObject>

class GuiTest : public QObject {
  Q_OBJECT

private slots:
  void runsOnTheOffscreenPlatform();
  void seesOnlyThePinnedFonts();
  void rendersTheForm();
  void rendersQuickInSoftware();
  void loadsOnlyPinnedLibraries();
};
