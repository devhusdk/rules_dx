let () =
  Dx_bench.run ~iterations:100
    [ ("sum", fun () -> ignore (Sys.opaque_identity (List.fold_left ( + ) 0 [ 1; 2; 3 ]))) ]
