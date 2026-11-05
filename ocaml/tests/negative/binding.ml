external add : (int[@untagged]) -> (int[@untagged]) -> (int[@untagged]) = "dx_add_bytecode" "dx_add" [@@noalloc]

let () = if add 1 2 <> 3 then exit 1
