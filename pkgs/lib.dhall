let FOPParams = { url : Text }

in  { mkPkg = λ(input : Bool) → input
    , mkFOP = λ(params : FOPParams) →  params
    }
