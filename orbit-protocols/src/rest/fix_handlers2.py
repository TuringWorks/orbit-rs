import re

with open('handlers.rs', 'r') as f:
    content = f.read()

# Replace multi-line Ok(( ... )) patterns
# Match Ok(( at start of line, followed by anything, then )) at end
content = re.sub(r'    Ok\(\(\n', r'    (\n', content)
content = re.sub(r'    \)\)\n\}', r'    )\n}', content)

with open('handlers.rs', 'w') as f:
    f.write(content)

print("Fixed multi-line Ok() wrapping")
