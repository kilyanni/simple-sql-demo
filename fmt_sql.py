#!/usr/bin/env python3
"""Format the migrations and the SQL strings in src/db/ with sqlfluff."""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RAW_STRING = re.compile(r'r#"\n(.*?)"#', re.DOTALL)


def fmt(sql: str) -> str:
    result = subprocess.run(
        ["sqlfluff", "fix", "--config", ROOT / ".sqlfluff", "-"],
        input=sql, capture_output=True, text=True, cwd=ROOT,
    )
    if result.returncode not in (0, 1) or not result.stdout.strip():
        raise SystemExit(result.stderr or result.stdout)
    return result.stdout.strip("\n") + "\n"


for path in ROOT.glob("migrations/*.sql"):
    path.write_text(fmt(path.read_text()))

for path in ROOT.glob("src/db/*.rs"):
    if path.name == "tests.rs":
        continue
    source = path.read_text()
    updated = RAW_STRING.sub(lambda m: 'r#"\n' + fmt(m.group(1)) + '"#', source)
    if updated != source:
        path.write_text(updated)
        print("formatted", path.relative_to(ROOT))
