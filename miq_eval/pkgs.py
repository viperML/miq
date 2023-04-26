from miq_eval.model import Package, Fetch


class bootstrap_source(Fetch):
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"


class busybox(Fetch):
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox"
    executable = True


class unpack_bootstrap_tools(Fetch):
    url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh"


class bootstrap(Package):
    name = "bootstrap"
    version = "0.1.0"
    deps = [
        bootstrap_source(),
        busybox(),
        unpack_bootstrap_tools(),
    ]
    env = {}

    def script_fn(self):
        return f"""
            set -exu
            {busybox()} mkdir -pv $miq_out
            pushd $miq_out
            {busybox()} tar -xvf {bootstrap_source()} --strip-components=1

            export out=$miq_out
            export builder={busybox()}

            {unpack_bootstrap_tools()}
        """
