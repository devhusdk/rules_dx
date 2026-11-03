let () =
  List.iter
    (fun (input, want) ->
      let got = Checksum.of_string input in
      if got <> want then (
        Printf.eprintf "checksum of %S: got %d, want %d\n" input got want;
        exit 1))
    [ ("", 0x811c9dc5); ("a", 0xe40c292c); ("foobar", 0xbf9cf968) ]
