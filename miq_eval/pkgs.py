from abc import ABC, abstractmethod
from miq_eval.model import Package, Fetch


class bootstrap_source(Fetch):
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"


class toybox(Fetch):
    url = "http://landley.net/toybox/bin/toybox-x86_64"
    executable = True


class busybox(Fetch):
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox"
    executable = True


class unpack_bootstrap_tools(Fetch):
    url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh"
    executable = True


class bootstrap(Package):
    name = "bootstrap"
    version = "0.1.0"
    deps = []
    env = {}

    def script_fn(self):
        return f"""
            set -exu
            {toybox()} mkdir -p $HOME/bin
            export PATH="$HOME/bin:${{PATH}}"
            {toybox()} ln -vs {toybox()} $HOME/bin/ln
            {toybox()} ln -vs {toybox()} $HOME/bin/cp
            {toybox()} ln -vs {toybox()} $HOME/bin/tar
            {toybox()} ln -vs {toybox()} $HOME/bin/mkdir
            {toybox()} ln -vs {toybox()} $HOME/bin/chmod

            cp -v {bootstrap_source()} $HOME/bootstrap.tar.xz
            mkdir -pv $miq_out
            pushd $miq_out
            tar -xvf $HOME/bootstrap.tar.xz

            export out=$miq_out
            export tarball={bootstrap_source()}
            export builder={busybox()}
            {unpack_bootstrap_tools()}
        """


class test1(Package):
    name = "test1"
    version = "0.0.0"
    deps = [
        toybox()
    ]
    env = {"PATH": f"{bootstrap()}/bin"}

    def script_fn(self):
        return """
            set -eux
            printenv > $miq_out
        """

class test2(Package):
    name = "test2"
    version = "0.0.0"
    deps = [
        test1()
    ]
    env = {"PATH": f"{bootstrap()}/bin"}

    def script_fn(self):
        return """
            set -eux
            printenv > $miq_out
        """
