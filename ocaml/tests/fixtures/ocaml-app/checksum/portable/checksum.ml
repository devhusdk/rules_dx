let step h b = ((h lxor b) * 16777619) land 0xffffffff

let of_string s =
  let h = ref 0x811c9dc5 in
  String.iter (fun c -> h := step !h (Char.code c)) s;
  !h
