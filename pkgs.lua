local miq = require "miq"

return {
    bootstrap = miq.fetch {
        url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
    },
    b2 = bootstrap
}
