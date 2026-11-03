let fail msg =
  prerr_endline msg;
  exit 1

let () =
  match Ledger.parse "# stock\nlamp 4 1250\n\ncable 12 199\n" with
  | Ok items ->
      let want = "lamp 4 x 1250 = 5000\ncable 12 x 199 = 2388\ntotal 7388" in
      if Ledger.report items <> want then fail ("unexpected report: " ^ Ledger.report items)
  | Error _ -> fail "a well-formed ledger did not parse"

let () =
  match Ledger.parse "lamp 4 1250\nlamp four 1\n" with
  | Error (Ledger.Malformed (2, _)) -> ()
  | _ -> fail "line 2 must be malformed"

let () =
  match Ledger.parse "bulb 5000 10\n" with
  | Error (Ledger.Over_limit "bulb") -> ()
  | _ -> fail "an oversized quantity must be over the limit"
