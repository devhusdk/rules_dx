let samples = 10

let escape s =
  let b = Buffer.create (String.length s) in
  String.iter
    (function
      | '"' -> Buffer.add_string b "\\\""
      | '\\' -> Buffer.add_string b "\\\\"
      | c -> Buffer.add_char b c)
    s;
  Buffer.contents b

let measure iterations f =
  f ();
  Array.init samples (fun _ ->
      let start = Unix.gettimeofday () in
      for _ = 1 to iterations do
        f ()
      done;
      (Unix.gettimeofday () -. start) *. 1e9 /. float_of_int iterations)

let stats xs =
  let n = float_of_int (Array.length xs) in
  let mean = Array.fold_left ( +. ) 0. xs /. n in
  let var =
    Array.fold_left (fun a x -> a +. ((x -. mean) *. (x -. mean))) 0. xs /. n
  in
  (mean, sqrt var)

let run ?(iterations = 1000) jobs =
  let results =
    List.map
      (fun (name, f) ->
        let mean, stddev = stats (measure iterations f) in
        Printf.printf "%s: %.17g ns/iter\n%!" name mean;
        Printf.sprintf
          "{\"name\":\"%s\",\"unit\":\"ns/iter\",\"mean\":%.17g,\"stddev\":%.17g,\"iterations\":%d}"
          (escape name) mean stddev (iterations * samples))
      jobs
  in
  let dir =
    match Sys.getenv_opt "TEST_UNDECLARED_OUTPUTS_DIR" with
    | Some d -> d
    | None -> "."
  in
  let oc = open_out (Filename.concat dir "dx-bench.json") in
  output_string oc
    ("{\"schema\":\"dx-bench/v1\",\"results\":["
    ^ String.concat "," results ^ "]}\n");
  close_out oc
