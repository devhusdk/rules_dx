let read_file path =
  let ic = open_in_bin path in
  let text = really_input_string ic (in_channel_length ic) in
  close_in ic;
  text

let usage () =
  prerr_endline "usage: stockctl FILE";
  exit 2

let () =
  match Array.to_list Sys.argv with
  | [ _; path ] -> (
      match Ledger.parse (read_file path) with
      | Ok items ->
          let report = Ledger.report items in
          print_endline report;
          Printf.printf "checksum %08x\n" (Checksum.of_string report)
      | Error (Ledger.Malformed (line, why)) ->
          Printf.eprintf "stockctl: line %d: %s\n" line why;
          exit 2
      | Error (Ledger.Over_limit name) ->
          Printf.eprintf "stockctl: %s is over the limit of %d\n" name Ledger.limit;
          exit 4)
  | _ -> usage ()
