"""Genera THIRD-PARTY-LICENSES.txt con los textos de licencia de todas las librerías de Rust incluidas.

Distribuir un binario obliga a conservar los avisos de copyright de MIT, Apache, BSD, etc.
Uso (desde desktop/, tras `cargo fetch`):  python3 tools/gen_licenses.py > THIRD-PARTY-LICENSES.txt
"""
import json
import pathlib
import subprocess

meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"], text=True))
workspace = set(meta["workspace_members"])
packages = sorted((p for p in meta["packages"] if p["id"] not in workspace), key=lambda p: (p["name"], p["version"]))

print("Licencias de terceros incluidas en Open control for Sphero Droids (escritorio)")
print(f"{len(packages)} librerías de Rust. Texto generado con tools/gen_licenses.py.")
print("De las que no incluyen archivo de licencia en su paquete se indica su licencia (SPDX) y su repositorio.\n")
for p in packages:
    d = pathlib.Path(p["manifest_path"]).parent
    files = sorted(f for f in d.iterdir() if f.is_file() and f.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "UNLICENSE", "NOTICE", "COPYRIGHT")))
    print("=" * 78)
    print(f"{p['name']} {p['version']}  —  {p.get('license') or 'ver el repositorio'}")
    if p.get("repository"):
        print(p["repository"])
    print()
    for f in files:
        print(f"--- {f.name} ---")
        print(f.read_text(errors="replace").strip())
        print()
