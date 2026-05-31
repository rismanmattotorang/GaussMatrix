{
  inputs,

  # Dependencies
  main,
  mdbook,
  stdenv,
}:

stdenv.mkDerivation {
  inherit (main) pname version;

  src = inputs.nix-filter {
    root = inputs.self;
    include = [
      "book.toml"
      "gaussmatrix-example.toml"
      "CODE_OF_CONDUCT.md"
      "CONTRIBUTING.md"
      "README.md"
      "development.md"
      "debian/gaussmatrix.service"
      "debian/README.md"
      "arch/gaussmatrix.service"
      "rpm/gaussmatrix.service"
      "rpm/README.md"
      "docs"
      "theme"
    ];
  };

  nativeBuildInputs = [
    mdbook
  ];

  buildPhase = ''
    mdbook build -d $out
  '';
}
