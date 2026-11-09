type t = Circle of float | Rect of float * float

let pi = 3.14159265358979

let area = function
  | Circle r -> pi *. r *. r
  | Rect (w, h) -> w *. h

let perimeter = function
  | Circle r -> 2. *. pi *. r
  | Rect (w, h) -> 2. *. (w +. h)

module Unit = struct
  let circle = Circle 1.
end
