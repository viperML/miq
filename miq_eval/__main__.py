import click

from miq_eval import pkgs
from miq_eval.model import Package, Fetch
from typing import Any
import sys


@click.command()
@click.argument("buildable")
@click.pass_context
def main(ctx: click.Context, **kwargs: dict[str, Any]):
    target: Package | Fetch = pkgs.__dict__[ctx.params["buildable"]]

    print(f"{repr(target)=}", file=sys.stderr)
    print(f"{str(target)}")


if __name__ == "__main__":
    main()
