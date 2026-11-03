(* Negative case: uses Astring without declaring the dependency, so it must not compile. *)
let first_word line = fst (Astring.String.span ~sat:(fun c -> c <> ' ') line)
