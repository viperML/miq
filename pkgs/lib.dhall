let FOP = { pname : Text, version : Text, url : Text }

in  { mkPkg = λ(input : Bool) → input, mkFOP = λ(params : FOP) → params }
