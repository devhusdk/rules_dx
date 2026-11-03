external step : (int[@untagged]) -> (int[@untagged]) -> (int[@untagged])
  = "dx_checksum_unavailable_in_bytecode" "dx_checksum_step"
[@@noalloc]

let of_string s =
  let h = ref 0x811c9dc5 in
  String.iter (fun c -> h := step !h (Char.code c)) s;
  !h
