from dataclasses import dataclass
from pathlib import Path
import struct
import hashlib
from urllib.parse import urlparse
from typing import List, Self, Callable, Any, Iterable, Iterator
import textwrap


def pyhash_to_miqhash(n: int) -> str:
    b = struct.pack("n", n)
    hasher = hashlib.sha1()
    hasher.update(b)
    return hasher.hexdigest()


def flatten(L: Iterable[Any]) -> Iterator[Any]:
    for item in L:
        try:
            yield from flatten(item)
        except TypeError:
            yield item


@dataclass(frozen=True)
class Fetch:
    url: str

    @property
    def name(self) -> str:
        return urlparse(self.url).path.split("/")[-1]

    @property
    def path_hash(self) -> str:
        return pyhash_to_miqhash(hash(self))

    @property
    def path(self) -> Path:
        return Path(f"/miq/store/{self.path_hash}-{self.name}")

    def to_spec(self) -> dict[str, Any]:
        return {"fetch": [{"path": str(self.path), "url": self.url, "hash": "FIXME"}]}

    def __str__(self) -> str:
        return str(self.path)


@dataclass(frozen=True)
class Package:
    name: str
    version: str
    script_fn: Callable[[Self], str]
    deps: List[Self | Fetch]
    env: dict[str, str]

    def __hash__(self) -> int:
        hashes = [
            hash(self.name),
            hash(self.version),
            hash(self.script),
            [hash(child) for child in self.deps],
            [hash(elem) for elem in self.env.keys()],
            [hash(elem) for elem in self.env.values()],
        ]

        hashes = [h for h in flatten(hashes)]

        return hash(frozenset(hashes))

    @property
    def path_hash(self) -> str:
        return pyhash_to_miqhash(hash(self))

    @property
    def path(self) -> Path:
        return Path(f"/miq/store/{self.path_hash}-{self.name}-{self.version}")

    @property
    def script(self) -> str:
        return textwrap.dedent(self.script_fn(self))

    def __str__(self) -> str:
        return str(self.path)

    # def to_spec(self) -> Tuple[dict[str, Any], List[dict[str, Any]]]:

    #     leaves = [x.to_spec() for x in self.deps]

    #     pass

    #     my_spec: dict[str, Any] = {
    #         "name": self.name,
    #         "path": str(self.path),
    #         "version": self.version,
    #         "script": self.script,
    #         "bdeps_buildm": map(lambda p: str(p), self.deps),
    #         "bdeps_hostm": [],
    #         "rdeps_hostm": [],
    #         "env": self.env,
    #     }

    #     return (my_spec, [])
    def to_spec(self) -> dict[str, Any]:
        return {}


def dump_package(input: Package) -> str:
    return "TODO"
