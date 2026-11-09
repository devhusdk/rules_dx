(** Geometric shapes. *)

(** A shape with its measurements. *)
type t =
  | Circle of float  (** A circle with its radius. *)
  | Rect of float * float  (** A rectangle with width and height. *)

val area : t -> float
(** [area shape] is the surface of [shape]. *)

val perimeter : t -> float
(** [perimeter shape] is the length around [shape]. *)

module Unit : sig
  val circle : t
  (** The circle of radius one. *)
end
