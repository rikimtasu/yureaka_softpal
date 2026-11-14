"""Fail when a sena diagnostic skipped a VM operation or stopped on a fault."""
import re
import sys
from collections import Counter
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="strict")
skipped = Counter(re.findall(r"unimplemented extcall (\S+)", text))
fatal = re.findall(r"(?:UnsupportedCommand|UnsupportedExtCall|Faulted|panicked|status=fault[^\n]*)", text)
print(f"Skipped operations: {sum(skipped.values())}; distinct: {len(skipped)}; faults: {len(fatal)}")
for name, count in skipped.most_common(20):
    print(f"  {name}: {count}")
if not re.search(r"\[(?:headless|window-diagnostic)\] frame=\d+", text):
    raise SystemExit("No diagnostic frames found")
raise SystemExit(bool(skipped or fatal))
