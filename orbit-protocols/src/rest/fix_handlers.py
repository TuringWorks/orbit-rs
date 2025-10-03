import re

with open('handlers.rs', 'r') as f:
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

with open('handlers.rs', 'w') as f:
    f.write(content)

print("Fixed handlers.rs")
