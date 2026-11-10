{ pkgs }:
let
  name = "dx";
in
{
  inherit name;
  shell = pkgs.mkShell { };
}
