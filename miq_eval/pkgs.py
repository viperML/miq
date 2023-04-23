from miq_eval.model import Package, Fetch
from miq_eval import model

bootstrap_source = Fetch(
    url="https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
)

bootstrap = Package(
    pname="bootstrap",
    version="0.1.0",
    deps=[
        #
        bootstrap_source
    ],
    script_fn=lambda self: """
        pwd
    """,
    env={},
)
