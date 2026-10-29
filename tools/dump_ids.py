"""Dump sphero::*CommandId / *DeviceId constants from libCore.so (ELF, ARM32)."""
import re
import sys

from elftools.elf.elffile import ELFFile

path = sys.argv[1]
pat = re.compile(r"^_ZN6sphero\d+(\w*?)(CommandId|DeviceId|Id)\d+(\w+)E$")

with open(path, "rb") as f:
    elf = ELFFile(f)
    out = []
    for secname in (".dynsym", ".symtab"):
        sec = elf.get_section_by_name(secname)
        if not sec:
            continue
        for s in sec.iter_symbols():
            m = pat.match(s.name)
            if not m or s["st_shndx"] in ("SHN_UNDEF", 0) or s["st_size"] == 0:
                continue
            addr = s["st_value"]
            data = None
            for seg in elf.iter_segments():
                if seg["p_type"] == "PT_LOAD" and seg["p_vaddr"] <= addr < seg["p_vaddr"] + seg["p_filesz"]:
                    f.seek(seg["p_offset"] + addr - seg["p_vaddr"])
                    data = f.read(s["st_size"])
                    break
            out.append((m.group(1) + m.group(2), m.group(3), s["st_size"], data.hex() if data else None))
    for row in sorted(set(out)):
        print(*row)
