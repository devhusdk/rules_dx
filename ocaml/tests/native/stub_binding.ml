(* LCOV_EXCL_START - reason: externals only, the native cell has no collector, issue: 8465 *)
external add : (int[@untagged]) -> (int[@untagged]) -> (int[@untagged])
  = "dx_add_unavailable_in_bytecode" "dx_add"
[@@noalloc]
(* LCOV_EXCL_STOP - reason: end of range, issue: 8465 *)
