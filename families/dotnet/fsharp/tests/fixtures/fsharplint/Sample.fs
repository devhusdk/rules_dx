// FSharpLint parse-vs-wire sample.
//
// Triggers two documented rules from the FSharpLint overview so the console
// fixtures below carry stable rule IDs:
// - FL0036 (InterfaceNamesMustBeginWithI) on `ExampleInterface`.
// - FL0034 (lambda removable, `( + )` partially applied) on the
//   `List.fold (fun x y -> x + y)` call (FL0065 `List.sum` fires only after
//   the FL0034 fix, so it stays out of the console pair).
module Sample

type ExampleInterface =
    abstract member print : unit -> unit

let total = List.fold (fun x y -> x + y) 0 [1; 2; 3]
