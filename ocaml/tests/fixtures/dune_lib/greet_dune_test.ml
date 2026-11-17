let () =
  let got = Farewell.bye "dune" in
  if got <> "HELLO DUNE! 42" then (prerr_endline got; exit 1)
