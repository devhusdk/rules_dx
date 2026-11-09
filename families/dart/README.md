# Dart and Flutter

Dart and Flutter test fixtures. They live in the `rules_dx_dart` module, next
to `rules_dx`. The rules, the tools, the lint, format and typecheck aspects and
the Gazelle language stay in `rules_dx`. The module fetches the pub packages and
the Flutter engines the fixtures use, so a workspace that does not use Dart or
Flutter does not fetch them.
