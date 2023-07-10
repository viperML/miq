{
  pkgs,
  config,
  ...
}: {
  packages.doc = pkgs.stdenvNoCC.mkDerivation {
    name = "miq-doc";
    nativeBuildInputs = [
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
          pdfpages
          setspace
          ;
      })
      pkgs.which
    ];

    src = ./.;

    buildPhase = ''
      runHook preBuild
      just clean build
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall

      mkdir -p $out
      cp -vL out/index.pdf $out

      runHook postInstall
    '';

    TEXMFHOME = "./cache";
    TEXMFVAR = "./cache/var";
  };

  packages.doc-dev = config.packages.doc.overrideAttrs (_: {
    src = null;
  });
}
