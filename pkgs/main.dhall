let lib = ./lib.dhall


in {
  foo = lib.mkFOP { url = "hello" }
}
