"""Genera src/catalog.rs (sonidos y animaciones de cada droide) a partir de las tablas CSV del APK.

Uso: python3 tools/gen_catalog.py <APK extraído>/assets/CMS/StaticData src/catalog.rs
"""
import csv
import sys

static_data, out_path = sys.argv[1], sys.argv[2]


def table(name):
    with open(f"{static_data}/{name}", newline="") as f:
        rows = list(csv.reader(f))
    header = rows[1]
    return [dict(zip(header, r)) for r in rows[2:] if r]


def sounds(droid_type):
    return [
        (d["Name"], int(d["Sector"]))
        for d in table("DroidSoundDefinitions.csv")
        if d["DroidType"] == droid_type and d["Enabled"] == "1" and int(d["Sector"]) >= 0
    ]


def animations(robot_type):
    out = []
    for d in table("RobotAnimatorAnimationDefinitions.csv"):
        if d["Robot Type"] == robot_type and d["Enabled"] == "1":
            name = d["Animation Name"].removesuffix(".bin")
            cat = d["Categorization"].strip().split("&")[0] or "Other"
            secs = float(d["Time"]) if d["Time"] else 3.0
            out.append((name, int(d["AnimationID"]), cat, secs))
    return out


lines = [
    "// Generado por tools/gen_catalog.py a partir de las tablas del APK. No editar a mano.",
    "",
    "use crate::kind::{Animation, Sound};",
    "",
]
for var, t in (("R2D2_SOUNDS", "R2D2"), ("BB9E_SOUNDS", "BB9E")):
    lines.append(f"pub static {var}: &[Sound] = &[")
    lines += [f'    Sound {{ name: "{n}", sector: {s} }},' for n, s in sounds(t)]
    lines += ["];", ""]
for var, t in (("R2D2_ANIMATIONS", "R2D2"), ("R2Q5_ANIMATIONS", "R2Q5"), ("BB9E_ANIMATIONS", "BB9E")):
    lines.append(f"pub static {var}: &[Animation] = &[")
    lines += [f'    Animation {{ name: "{n}", index: {i}, category: "{c}", seconds: {s} }},' for n, i, c, s in animations(t)]
    lines += ["];", ""]

with open(out_path, "w") as f:
    f.write("\n".join(lines))
print("ok")
