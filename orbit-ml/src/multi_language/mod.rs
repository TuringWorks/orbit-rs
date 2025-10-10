#[cfg(feature = "python")]
pub mod python {
    pub struct PythonMLEngine;
}

#[cfg(feature = "javascript")]
pub mod javascript {
    pub struct JavaScriptMLEngine;
}

#[cfg(feature = "lua")]
pub mod lua {
    pub struct LuaMLEngine;
}
