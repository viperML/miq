import hashlib
import struct
import textwrap
from abc import ABC, abstractmethod, abstractproperty
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable, Iterator, List, Self
from urllib.parse import urlparse

import toml

HASHER = hashlib.sha1


# def pyhash_to_miqhash(n: int) -> str:
#     b = struct.pack("n", n)
#     hasher = hashlib.sha1()
#     hasher.update(b)
#     return hasher.hexdigest()


def flatten(L: Iterable[Any]) -> Iterator[Any]:
    for item in L:
        try:
            yield from flatten(item)
        except TypeError:
            yield item


@dataclass
class Unit(ABC):
    def __post_init__(self):
        path = Path(f"/miq/eval/{self.result}.toml")
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w") as f:
            # f.write("#:schema /miq/eval-schema.json\n")
            f.write(toml.dumps(self.to_spec()))

    @property
    def result(self) -> str:
        raise NotImplementedError

    def __str__(self) -> str:
        return self.result

    @abstractproperty
    def hash(self) -> str:
        raise NotImplementedError

    @abstractmethod
    def to_spec(self) -> dict[str, Any]:
        raise NotImplementedError


@dataclass
class Fetch(Unit):
    url: str

    @property
    def name(self) -> str:
        return urlparse(self.url).path.split("/")[-1]

    @property
    def result(self) -> str:
        return f"{self.name}-{self.hash}"

    @property
    def hash(self) -> str:
        h = HASHER()
        h.update(self.url.encode())
        return h.hexdigest()

    def to_spec(self) -> dict[str, Any]:
        return {
            "result": self.result,
            "name": self.name,
            "url": self.url,
            "integrity": "FIXME",
        }


@dataclass(frozen=False)
class Package(Unit):
    name: str
    version: str
    script_fn: Callable[[Self], str]
    deps: List[Self | Fetch]
    env: dict[str, str]

    @property
    def result(self) -> str:
        return f"{self.name}-{self.version}-{self.hash}"

    @property
    def hash(self) -> str:
        elems = [
            self.name,
            self.version,
            self.script,
            *[elem for elem in self.env.keys()],
            *[elem for elem in self.env.values()],
        ]

        h = HASHER()
        for elem in elems:
            elem = elem.encode()
            h.update(elem)

        return h.hexdigest()

    def to_spec(self) -> dict[str, Any]:
        return {
            "result": self.result,
            "name": self.name,
            "version": self.version,
            "script": self.script,
            "deps": [str(d) for d in self.deps],
            "env": self.env,
        }

    @property
    def script(self) -> str:
        return textwrap.dedent(self.script_fn(self))