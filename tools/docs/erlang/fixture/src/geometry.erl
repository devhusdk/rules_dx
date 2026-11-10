%% @doc Area helpers for simple shapes.
-module(geometry).
-export([area/2, perimeter/2]).

%% @doc Returns the area of a rectangle.
-spec area(number(), number()) -> number().
area(Width, Height) -> Width * Height.

%% @doc Returns the perimeter of a rectangle.
-spec perimeter(number(), number()) -> number().
perimeter(Width, Height) -> 2 * (Width + Height).

%% @doc Not exported, so not documented.
hidden() -> ok.
