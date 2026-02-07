// For now everything is text for universal

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub params_text: String,
    pub return_type: Option<String>,
}

#[derive(Debug)]
pub struct Type {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug)]
pub struct Import {
    pub path: String,
    pub name: String,
    pub alias: Option<String>,
    pub is_wildcard: bool,
}

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub type_text: Option<String>,
    pub value: Option<String>,
    pub is_mut: bool,
    pub is_const: bool,
    pub is_static: bool,
}
