let () =
  if Greet.hello "test" <> "HELLO, TEST" then (
    prerr_endline "unexpected greeting";
    exit 1)
