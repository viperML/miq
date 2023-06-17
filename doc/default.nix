{pkgs, ...}: {
  devShells.doc = pkgs.mkShellNoCC {
    packages = [
      pkgs.just
      (pkgs.texlive.combine {
        inherit
          (pkgs.texlive)
          scheme-medium
          biblatex
          biber
          svg
          trimspaces
          transparent
          lipsum
          ;
      })
    ];
  };
}
