import click

from miq_eval import pkgs
from miq_eval.model import Package, Fetch
import toml


@click.command()
@click.argument("buildable")
def main(buildable: str):
    target: Package | Fetch = pkgs.__dict__[buildable]

    print(target.__repr__())
    print(target.path_hash)
    print(toml.dumps( target.to_spec()))


    pass


if __name__ == "__main__":
    main()
