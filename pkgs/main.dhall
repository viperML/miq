let lib = ./lib.dhall

in  { zig =
        lib.mkFOP
          { url =
              "https://ziglang.org/builds/zig-linux-x86_64-0.11.0-dev.1929+4ea2f441d.tar.xz"
          , pname = "zig"
          , version = "0.11.0"
          }
    }
