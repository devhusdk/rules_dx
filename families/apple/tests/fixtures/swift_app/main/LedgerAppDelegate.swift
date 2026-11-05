import Foundation
import LedgerCore
import LedgerKit
import UIKit

@main
final class LedgerAppDelegate: UIResponder, UIApplicationDelegate {
  var window: UIWindow?

  func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    let root = UIViewController()
    let label = UILabel(frame: CGRect(x: 20, y: 80, width: 280, height: 40))
    root.view.addSubview(label)
    let window = UIWindow(frame: UIScreen.main.bounds)
    window.rootViewController = root
    window.makeKeyAndVisible()
    self.window = window

    guard let path = Bundle.main.path(forResource: "entries", ofType: "txt"),
      let text = try? String(contentsOfFile: path, encoding: .utf8),
      let ledger = Ledger(text: text)
    else {
      return true
    }
    let total = ledger_total(text)
    label.text = "\(total) cents"
    guard total == ledger.total else {
      return true
    }
    let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    let state = ledger.state(platform: String(cString: ledger_platform()), total: total)
    try? state.write(
      to: documents.appendingPathComponent("ledger-state.txt"), atomically: true, encoding: .utf8)
    return true
  }
}
