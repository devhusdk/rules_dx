let () =
  if Greet.greet "world" <> "goodbye world" then (
    prerr_endline "failing_case";
    exit 1)
