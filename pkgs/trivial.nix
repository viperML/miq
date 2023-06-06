{pkgs, ...}: {
  packages.trivial = with pkgs.pkgsMusl;
    stdenv.mkDerivation {
      name = "trivial";
      src = ./pkgs;
      NIX_DEBUG = "1";

      buildPhase = ''
        $CC trivial.c -o $out
      '';

      dontInstall = true;
    };
}
