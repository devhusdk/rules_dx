let () =
  if Stub_binding.add 2 3 <> 5 then (
    prerr_endline "the C stub returned the wrong sum";
    exit 1)

let () =
  let ic = open_in "ocaml/tests/native/data.txt" in
  let line = input_line ic in
  close_in ic;
  if line <> "native data" then (
    prerr_endline ("unexpected data: " ^ line);
    exit 1)
