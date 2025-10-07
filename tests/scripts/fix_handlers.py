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

# Replace ProtocolResult<impl IntoResponse> with impl IntoResponse
content = re.sub(r'\) -> ProtocolResult<impl IntoResponse> \{', r') -> impl IntoResponse {', content)

# Fix return statements - replace Ok((...)) with just (...)  
# But be careful to only match the return statements
content = re.sub(r'    Ok\(\(StatusCode::', r'    (StatusCode::', content)

# Fix final closing parens on return lines
content = re.sub(r'\)\)\)$', r'))', content, flags=re.MULTILINE)

# Fix the openapi_spec return
content = re.sub(r'Json\(ApiDoc::openapi\(\)$', r'Json(ApiDoc::openapi())', content, flags=re.MULTILINE)

with open(handlers_path, 'w') as f:
    f.write(content)

print(f"Fixed {handlers_path}")
