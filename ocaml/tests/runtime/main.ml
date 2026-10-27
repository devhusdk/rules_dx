let () =
  let ic = open_in "ocaml/tests/runtime/data.txt" in
  let name = String.trim (input_line ic) in
  close_in ic;
  print_endline (Greet.hello name)
