"""Genera Catalog.swift (sonidos y animaciones de cada droide) a partir de las tablas CSV del APK."""
import csv
import sys

static_data, out_path = sys.argv[1], sys.argv[2]


def table(name):
    with open(f"{static_data}/{name}", newline="") as f:
        rows = list(csv.reader(f))
    header = rows[1]
    return [dict(zip(header, r)) for r in rows[2:] if r]


def sounds(droid_type):
    out = []
    for d in table("DroidSoundDefinitions.csv"):
        if d["DroidType"] == droid_type and d["Enabled"] == "1" and int(d["Sector"]) >= 0:
            out.append((d["Name"], int(d["Sector"]), d["RandomChatter"].upper() == "TRUE"))
    return out


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
    "struct DroidSound: Identifiable { let name: String; let sector: UInt16; let chatter: Bool; var id: UInt16 { sector } }",
    "struct DroidAnimation: Identifiable { let name: String; let index: UInt16; let category: String; let seconds: Double; var id: UInt16 { index } }",
    "",
    "enum Catalog {",
]
for var, t in (("r2d2Sounds", "R2D2"), ("bb9eSounds", "BB9E")):
    lines.append(f"    static let {var}: [DroidSound] = [")
    lines += [f'        DroidSound(name: "{n}", sector: {s}, chatter: {str(c).lower()}),' for n, s, c in sounds(t)]
    lines.append("    ]")
for var, t in (("r2d2Animations", "R2D2"), ("r2q5Animations", "R2Q5"), ("bb9eAnimations", "BB9E")):
    lines.append(f"    static let {var}: [DroidAnimation] = [")
    lines += [f'        DroidAnimation(name: "{n}", index: {i}, category: "{c}", seconds: {s}),' for n, i, c, s in animations(t)]
    lines.append("    ]")
lines += ["}", ""]

with open(out_path, "w") as f:
    f.write("\n".join(lines))
print("ok")
