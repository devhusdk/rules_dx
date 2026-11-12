type item = {
  name : string;
  qty : int;
  cents : int;
}

type error =
  | Malformed of int * string
  | Over_limit of string

val limit : int
(** The largest quantity one line may hold. *)

val parse : string -> (item list, error) result
(** Parses lines of the form [NAME QTY CENTS]; blank lines and [#] comments are skipped. *)

val line_total : item -> int
val report : item list -> string
