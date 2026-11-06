let () =
  let got = Greet.hello "dune" in
  if got <> "HELLO DUNE!" then (prerr_endline got; exit 1)
