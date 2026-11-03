type item = {
  name : string;
  qty : int;
  cents : int;
}

type error =
  | Malformed of int * string
  | Over_limit of string

let limit = 1000

let parse_line number line =
  match Tokens.tokens_of_string line with
  | Error msg -> Error (Malformed (number, msg))
  | Ok [ Tokens.Name name; Tokens.Number qty; Tokens.Number cents ] ->
      if qty > limit then Error (Over_limit name) else Ok { name; qty; cents }
  | Ok _ -> Error (Malformed (number, "expected NAME QTY CENTS"))

let skipped line =
  let line = String.trim line in
  line = "" || Astring.String.is_prefix ~affix:"#" line

let parse text =
  let lines = Astring.String.cuts ~sep:"\n" text in
  let rec loop number acc = function
    | [] -> Ok (List.rev acc)
    | line :: rest when skipped line -> loop (number + 1) acc rest
    | line :: rest -> (
        match parse_line number line with
        | Ok item -> loop (number + 1) (item :: acc) rest
        | Error _ as e -> e)
  in
  loop 1 [] lines

let line_total item = item.qty * item.cents

let report items =
  let lines =
    List.map
      (fun i -> Printf.sprintf "%s %d x %d = %d" i.name i.qty i.cents (line_total i))
      items
  in
  let total = List.fold_left (fun acc i -> acc + line_total i) 0 items in
  String.concat "\n" (lines @ [ Printf.sprintf "total %d" total ])
