let lib = ./lib.dhall

in  { foo = lib.mkFOP { url = "https://github", pname = "hello", version = "1.0" } }
