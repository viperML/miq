from copy import deepcopy
import hashlib
import textwrap
from abc import ABC, abstractmethod, abstractproperty
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable, Iterator, List, Self, Set
from urllib.parse import urlparse
import re

import toml

HASHER = hashlib.sha1

# Magic character for string interpolation
MGC = "ᛈ"
PATTERN = re.compile(r"(?<=ᛈ>).*?(?=<ᛈ)")


def eval_string(input: str) -> tuple[str, List[str]]:
    mgc_len = len(MGC)
    extra_deps: set[str] = set()

    while elems := [e for e in re.finditer(PATTERN, input)]:
        elem = elems[0]
        result = elem.group()
        extra_deps.add(result)

        start = elem.start() - mgc_len - 1
        end = elem.end() + mgc_len + 1

        input = input[:start] + f"/miq/store/{result}" + input[end:]

    return (input, list(extra_deps))


if __name__ == "__main__":
    print(eval_string("Helᛈᛈoᛈᛈ World"))


def flatten(L: Iterable[Any]) -> Iterator[Any]:
    for item in L:
        try:
            yield from flatten(item)
        except TypeError:
            yield item


@dataclass(init=False)
class Unit(ABC):
    def __init__(self):
        path = Path(f"/miq/eval/{self.result}.toml")
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w") as f:
            # f.write("#:schema /miq/eval-schema.json\n")
            f.write(toml.dumps(self.to_spec()))

    @property
    def result(self) -> str:
        raise NotImplementedError

    def __str__(self) -> str:
        # return f"/miq/store/{self.result}"
        return f"{MGC}>{self.result}<{MGC}"

    @abstractproperty
    def hash(self) -> str:
        raise NotImplementedError

    @abstractmethod
    def to_spec(self) -> dict[str, Any]:
        raise NotImplementedError


@dataclass(init=False)
class Fetch(Unit):
    url: str
    executable: bool = False

    @property
    def name(self) -> str:
        return urlparse(self.url).path.split("/")[-1]

    @property
    def result(self) -> str:
        return f"{self.name}-{self.hash}"
        # return f"{self.hash}-{self.name}"

    @property
    def hash(self) -> str:
        h = HASHER()
        h.update(self.url.encode())
        h.update(bytes(self.executable))
        return h.hexdigest()

    def to_spec(self) -> dict[str, Any]:
        return {
            "result": self.result,
            "name": self.name,
            "url": self.url,
            "executable": self.executable,
            "integrity": "FIXME",
        }


@dataclass(init=False)
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
        elems = {
            self.name,
            self.version,
            self.script,
            *[elem for elem in self._env.keys()],
            *[elem for elem in self._env.values()],
            *self._deps
        }

        h = HASHER()
        for elem in elems:
            elem = elem.encode()
            h.update(elem)

        return h.hexdigest()

    @property
    def _deps(self) -> List[str]:
        result: Set[str] = set()
        for d in self.deps:
            if isinstance(d, str):
                result.add(d)
            else:
                result.add(d.result)
        return list(result)

    @property
    def _env(self) -> dict[str, str]:
        result: dict[str, str] = {}
        for k, v in self.env.items():
            text, extra_deps = eval_string(v)
            result[k] = text
            self.deps.extend(extra_deps)  # type:ignore
        return result

    def to_spec(self) -> dict[str, Any]:
        return {
            "result": self.result,
            "name": self.name,
            "version": self.version,
            "script": self.script,
            "env": self._env,
            # last
            "deps": self._deps,
        }

    @property
    def script(self) -> str:
        text: str = self.script_fn()  # type: ignore
        final_text, extra_deps = eval_string(text)  # type: ignore
        self.deps.extend(extra_deps)  # type: ignore
        return textwrap.dedent(final_text)  # type: ignore
