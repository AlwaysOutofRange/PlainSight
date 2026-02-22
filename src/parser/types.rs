use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub name: String,
    pub params_text: String,
    pub return_type: Option<String>,
    pub visibility: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Type {
    pub name: String,
    pub kind: Option<String>,
    pub visibility: Option<String>,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Import {
    pub path: String,
    pub name: String,
    pub alias: Option<String>,
    pub is_wildcard: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Variable {
    pub name: String,
    pub type_text: Option<String>,
    pub value: Option<String>,
    pub visibility: Option<String>,
    pub is_mut: bool,
    pub is_const: bool,
    pub is_static: bool,
}
