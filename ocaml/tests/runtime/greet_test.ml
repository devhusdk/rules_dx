let () =
  if Greet.hello "test" <> "HELLO, TEST" then (
    prerr_endline "unexpected greeting";
    exit 1)

let () =
  if Sys.getenv_opt "DX_OCAML_PROBE" <> Some "env" then (
    prerr_endline "env did not reach the test";
    exit 1)
