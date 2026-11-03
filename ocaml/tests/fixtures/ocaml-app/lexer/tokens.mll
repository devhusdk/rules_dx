{
type token =
  | Name of string
  | Number of int
  | Eof
}

let blank = [' ' '\t']+
let name = ['a' 'b' 'c' 'd' 'e' 'f' 'g' 'h' 'i' 'j' 'k' 'l' 'm' 'n' 'o' 'p' 'q' 'r' 's' 't' 'u' 'v' 'w' 'x' 'y' 'z' '_']+
let number = ['0'-'9']+

rule token = parse
  | blank { token lexbuf }
  | name as n { Name n }
  | number as d { Number (int_of_string d) }
  | eof { Eof }

{
let tokens_of_string s =
  let lexbuf = Lexing.from_string s in
  let rec loop acc =
    match token lexbuf with
    | Eof -> Ok (List.rev acc)
    | t -> loop (t :: acc)
  in
  try loop [] with
  | Failure _ ->
      Error (Printf.sprintf "unexpected character at column %d" (Lexing.lexeme_start lexbuf + 1))
}
