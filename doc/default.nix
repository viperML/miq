{pkgs, ...}: {
  devShells.doc = pkgs.mkShellNoCC {
    packages = [
      pkgs.just
      (pkgs.python3.withPackages (p: [
        p.pygments
      ]))
      pkgs.inkscape
      pkgs.graphviz-nox
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
          acronym
          bigfoot
          xstring
          minted
          placeins
          tabularray
ninecolors
xurl
          ;
      })
    ];
  };
}
