import click

from miq_eval import pkgs
from miq_eval.model import Package, Fetch
from typing import Any
from sys import stderr


@click.command()
@click.argument("unit")
@click.pass_context
def main(ctx: click.Context, **kwargs: dict[str, Any]):
    unit: Package | Fetch = pkgs.__dict__[ctx.params["unit"]]()

    print(f"{repr(unit)=}", file=stderr)
    print(f"{unit.result}")


if __name__ == "__main__":
    main()
