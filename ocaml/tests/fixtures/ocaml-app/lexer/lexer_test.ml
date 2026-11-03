let fail msg =
  prerr_endline msg;
  exit 1

let () =
  match Tokens.tokens_of_string "lamp 4  1250" with
  | Ok [ Tokens.Name "lamp"; Tokens.Number 4; Tokens.Number 1250 ] -> ()
  | _ -> fail "a well-formed line did not tokenize"

let () =
  match Tokens.tokens_of_string "lamp 4 $5" with
  | Error msg when String.length msg > 0 -> ()
  | Error _ -> fail "the error must say what went wrong"
  | Ok _ -> fail "malformed input must return an Error"
