from miq_eval.model import Package, Fetch


class bootstrap_source(Fetch):
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"


class bootstrap(Package):
    name = "bootstrap"
    version = "0.1.0"
    deps = [
        #
        bootstrap_source()
    ]
    env = {}

    def script_fn(self):
        return """
            # pwd
        """
