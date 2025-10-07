import re
import os
from pathlib import Path

# Find handlers.rs in the orbit-protocols/src/rest directory
handlers_path = Path(__file__).parent.parent.parent / "orbit-protocols" / "src" / "rest" / "handlers.rs"

if not handlers_path.exists():
    print(f"Error: Could not find handlers.rs at {handlers_path}")
    exit(1)

with open(handlers_path, 'r') as f:
    content = f.read()

# Replace multi-line Ok(( ... )) patterns
# Match Ok(( at start of line, followed by anything, then )) at end
content = re.sub(r'    Ok\(\(\n', r'    (\n', content)
content = re.sub(r'    \)\)\n\}', r'    )\n}', content)

with open(handlers_path, 'w') as f:
    f.write(content)

print(f"Fixed multi-line Ok() wrapping in {handlers_path}")
